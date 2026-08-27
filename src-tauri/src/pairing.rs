use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_DIR: &str = "aerodesk";
const CONFIG_FILE: &str = "config.json";
pub const DEFAULT_PORT: u16 = 0;

#[derive(Serialize, Deserialize, Clone)]
pub struct PairingConfig {
    pub token: String,
    pub port: u16,
    pub host: String,
    pub name: String,
    #[serde(default)]
    pub host_manual: bool,
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join(CONFIG_DIR).join(CONFIG_FILE))
}

pub fn load_or_init() -> PairingConfig {
    if let Some(p) = config_path() {
        if let Ok(s) = fs::read_to_string(&p) {
            if let Ok(mut cfg) = serde_json::from_str::<PairingConfig>(&s) {
                if !cfg.token.is_empty() && !cfg.host.is_empty() {
                    refresh_lan_ip(&mut cfg);
                    return cfg;
                }
            }
        }
    }
    let cfg = PairingConfig {
        token: gen_token(),
        port: DEFAULT_PORT,
        host: detect_lan_ip(),
        name: detect_hostname(),
        host_manual: false,
    };
    save(&cfg);
    cfg
}

fn refresh_lan_ip(cfg: &mut PairingConfig) {
    if cfg.host_manual {
        return;
    }
    let ip = detect_lan_ip();
    if ip != "127.0.0.1" && ip != cfg.host {
        cfg.host = ip;
        save(cfg);
    }
}

pub fn save(cfg: &PairingConfig) {
    if let Some(p) = config_path() {
        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(s) = serde_json::to_string_pretty(cfg) {
            let _ = fs::write(&p, s);
        }
    }
}

pub fn gen_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes)
}

fn detect_lan_ip() -> String {
    local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

fn detect_hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "AeroDesk".to_string())
}

/// `ws://host:port` — token is NOT in the URL; auth happens via the first WS
/// message (see ws_server.rs) so it never appears in a URL/query log.
pub fn ws_url(cfg: &PairingConfig) -> String {
    format!("ws://{}:{}", cfg.host, cfg.port)
}

/// JSON payload encoded into the QR code. The phone scans + parses this.
pub fn qr_payload(cfg: &PairingConfig) -> String {
    serde_json::json!({
        "v": 1,
        "url": ws_url(cfg),
        "token": cfg.token,
        "name": cfg.name,
    })
    .to_string()
}

/// Render the payload as an inline SVG string (hand-rolled from the module
/// matrix — avoids pulling the `image` crate).
#[allow(deprecated)] // to_vec() is deprecated in favor of to_colors() but returns Vec<bool> directly
pub fn qr_svg(payload: &str) -> Result<String, String> {
    let code = qrcode::QrCode::new(payload.as_bytes()).map_err(|e| e.to_string())?;
    let modules = code.to_vec();
    let n = code.width();
    const SCALE: u32 = 8;
    const QUIET: u32 = 4;
    let dim = (n as u32 + QUIET * 2) * SCALE;
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{dim}\" height=\"{dim}\" viewBox=\"0 0 {dim} {dim}\" shape-rendering=\"crispEdges\">");
    s.push_str(&format!("<rect width=\"{dim}\" height=\"{dim}\" fill=\"#ffffff\"/>"));
    for y in 0..n {
        for x in 0..n {
            if modules[y * n + x] {
                let px = (x as u32 + QUIET) * SCALE;
                let py = (y as u32 + QUIET) * SCALE;
                s.push_str(&format!(
                    "<rect x=\"{px}\" y=\"{py}\" width=\"{sc}\" height=\"{sc}\" fill=\"#000000\"/>",
                    sc = SCALE
                ));
            }
        }
    }
    s.push_str("</svg>");
    Ok(s)
}