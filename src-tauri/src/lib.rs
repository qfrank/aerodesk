mod inject;
mod pairing;
mod platform;
mod ws_server;

use std::sync::{Arc, RwLock};

use pairing::PairingConfig;
use tauri::{Manager, State};
use ws_server::DeviceRegistry;

struct AppState {
    cfg: Arc<RwLock<PairingConfig>>,
    devices: DeviceRegistry,
}

fn build_info(cfg: &PairingConfig) -> serde_json::Value {
    let payload = pairing::qr_payload(cfg);
    let qr = pairing::qr_svg(&payload).unwrap_or_default();
    serde_json::json!({
        "url": pairing::ws_url(cfg),
        "token": cfg.token,
        "host": cfg.host,
        "port": cfg.port,
        "name": cfg.name,
        "qrSvg": qr,
    })
}

#[tauri::command]
fn get_pairing_info(state: State<AppState>) -> serde_json::Value {
    build_info(&state.cfg.read().unwrap())
}

#[tauri::command]
fn set_host(host: String, state: State<AppState>) -> serde_json::Value {
    {
        let mut cfg = state.cfg.write().unwrap();
        cfg.host = host.trim().to_string();
        pairing::save(&cfg);
    }
    build_info(&state.cfg.read().unwrap())
}

#[tauri::command]
fn check_accessibility() -> bool {
    inject::is_accessibility_trusted()
}

#[tauri::command]
fn get_devices(state: State<AppState>) -> Vec<ws_server::DeviceState> {
    let reg = state.devices.read().unwrap();
    let mut devs: Vec<ws_server::DeviceState> = reg.values().cloned().collect();
    devs.sort_by(|a, b| a.name.cmp(&b.name));
    devs
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("aerodesk=info".parse().unwrap()),
        )
        .try_init();

    let registry = ws_server::new_registry();
    tauri::Builder::default()
        .manage(AppState {
            cfg: Arc::new(RwLock::new(pairing::load_or_init())),
            devices: registry.clone(),
        })
        .invoke_handler(tauri::generate_handler![
            get_pairing_info,
            set_host,
            check_accessibility,
            get_devices
        ])
        .setup(|app| {
            let st = app.state::<AppState>();
            let cfg = st.inner().cfg.clone();
            // Bind the WS port before the window loads so the QR / ws URL carry
            // the real port. Preferred port comes from the persisted config; 0
            // (or a taken port) falls back to an OS-assigned free port.
            let preferred = cfg.read().unwrap().port;
            let (listener, actual_port) =
                match tauri::async_runtime::block_on(ws_server::bind_listener(preferred)) {
                    Ok(v) => v,
                    Err(e) => return Err(e.into()),
                };
            {
                let mut c = cfg.write().unwrap();
                if c.port != actual_port {
                    c.port = actual_port;
                    pairing::save(&c);
                }
            }
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                ws_server::serve(listener, cfg, registry, handle).await;
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running AeroDesk");
}