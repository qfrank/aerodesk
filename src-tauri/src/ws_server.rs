use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::pairing::PairingConfig;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(20);

static CONN_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, serde::Serialize)]
pub struct DeviceState {
    pub name: String,
    pub connected: bool,
    #[serde(skip)]
    pub conn_id: u64,
}

pub type DeviceRegistry = Arc<RwLock<HashMap<String, DeviceState>>>;

pub fn new_registry() -> DeviceRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Bind the WS listener. If `preferred` is 0 (or the port is already taken),
/// fall back to an OS-assigned free port (`bind 0.0.0.0:0`) so we never collide
/// with other apps on a fixed port. Returns the listener + the actually-bound
/// port (so the caller can persist it + put it in the QR).
pub async fn bind_listener(preferred: u16) -> Result<(TcpListener, u16), String> {
    if preferred != 0 {
        if let Ok(l) = TcpListener::bind(("0.0.0.0", preferred)).await {
            tracing::info!("AeroDesk WS listening on 0.0.0.0:{preferred}");
            return Ok((l, preferred));
        }
        tracing::warn!("AeroDesk WS bind 0.0.0.0:{preferred} failed, falling back to OS-assigned port");
    }
    let l = TcpListener::bind(("0.0.0.0", 0u16))
        .await
        .map_err(|e| format!("bind 0.0.0.0:0 failed: {e}"))?;
    let port = l.local_addr().map_err(|e| format!("local_addr: {e}"))?.port();
    tracing::info!("AeroDesk WS listening on 0.0.0.0:{port} (OS-assigned)");
    Ok((l, port))
}

pub async fn serve(
    listener: TcpListener,
    cfg: Arc<RwLock<PairingConfig>>,
    registry: DeviceRegistry,
    app: AppHandle,
) {
    while let Ok((stream, _addr)) = listener.accept().await {
        let cfg = cfg.clone();
        let registry = registry.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = handle_conn(stream, cfg, registry, app).await {
                tracing::warn!("AeroDesk WS conn error: {e}");
            }
        });
    }
}

async fn handle_conn(
    stream: tokio::net::TcpStream,
    cfg: Arc<RwLock<PairingConfig>>,
    registry: DeviceRegistry,
    app: AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut ws = tokio_tungstenite::accept_async(stream).await?;

    // First message must be auth: {"type":"auth","token":"...","name":"..."}.
    let auth_msg = match ws.next().await {
        Some(Ok(Message::Text(t))) => t,
        _ => {
            let _ = ws.send(Message::Close(None)).await;
            return Ok(());
        }
    };
    let auth: serde_json::Value = match serde_json::from_str(&auth_msg) {
        Ok(v) => v,
        Err(_) => {
            let _ = ws.send(Message::Close(None)).await;
            return Ok(());
        }
    };
    let expected = cfg.read().unwrap().token.clone();
    let phone_name = auth
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("phone")
        .to_string();
    if auth.get("type").and_then(|v| v.as_str()) != Some("auth")
        || auth.get("token").and_then(|v| v.as_str()) != Some(&expected)
    {
        let _ = ws
            .send(Message::Text(
                r#"{"type":"ack","ok":false,"error":"bad token"}"#.to_string(),
            ))
            .await;
        let _ = ws.send(Message::Close(None)).await;
        return Ok(());
    }

    let conn_id = CONN_SEQ.fetch_add(1, Ordering::Relaxed);
    {
        let mut reg = registry.write().unwrap();
        reg.insert(
            phone_name.clone(),
            DeviceState {
                name: phone_name.clone(),
                connected: true,
                conn_id,
            },
        );
    }
    let _ = app.emit(
        "aerodesk://device-connected",
        serde_json::json!({ "name": phone_name }),
    );

    let (sink, mut stream) = ws.split();
    let (tx, rx) = mpsc::unbounded_channel::<Message>();
    let send_task = tauri::async_runtime::spawn(async move {
        let mut rx = rx;
        let mut sink = sink;
        while let Some(msg) = rx.recv().await {
            if sink.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut last_recv = Instant::now();
    let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(t))) => {
                        last_recv = Instant::now();
                        let v: serde_json::Value = match serde_json::from_str(&t) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        match v.get("type").and_then(|x| x.as_str()) {
                            Some("ping") => {
                                let _ = tx.send(Message::Text(
                                    r#"{"type":"pong"}"#.to_string(),
                                ));
                            }
                            Some("pong") => {}
                            Some("text") => {
                                let text = v
                                    .get("text")
                                    .and_then(|x| x.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let session_id = v
                                    .get("sessionId")
                                    .and_then(|x| x.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let _ = app.emit(
                                    "aerodesk://text-received",
                                    serde_json::json!({
                                        "name": phone_name,
                                        "text": text,
                                        "ts": now_millis(),
                                    }),
                                );
                                let result = crate::inject::inject_text(&text);
                                let ack = match &result {
                                    Ok(_) => {
                                        r#"{"type":"ack","ok":true}"#.to_string()
                                    }
                                    Err(e) => {
                                        let _ = app.emit(
                                            "aerodesk://send-error",
                                            serde_json::json!({
                                                "sessionId": session_id,
                                                "error": e,
                                            }),
                                        );
                                        format!(
                                            r#"{{"type":"ack","ok":false,"error":{}}}"#,
                                            serde_json::Value::String(e.clone())
                                        )
                                    }
                                };
                                let _ = tx.send(Message::Text(ack));
                            }
                            _ => {}
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => continue,
                }
            }
            _ = interval.tick() => {
                let _ = tx.send(Message::Text(r#"{"type":"ping"}"#.to_string()));
                if last_recv.elapsed() > HEARTBEAT_TIMEOUT {
                    break;
                }
            }
        }
    }

    drop(tx);
    let _ = send_task.await;

    let do_emit = {
        let mut reg = registry.write().unwrap();
        if let Some(dev) = reg.get_mut(&phone_name) {
            if dev.conn_id == conn_id && dev.connected {
                dev.connected = false;
                true
            } else {
                false
            }
        } else {
            false
        }
    };
    if do_emit {
        let _ = app.emit(
            "aerodesk://device-disconnected",
            serde_json::json!({ "name": phone_name }),
        );
    }
    Ok(())
}