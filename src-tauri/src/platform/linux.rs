//! Linux: clipboard-free CJK insertion has no single universal API. v2 uses
//! AT-SPI (the Linux accessibility bus) as the primary path — it works on both
//! X11 and Wayland and injects directly into the focused editable control via
//! `EditableText.InsertText`, independent of IME. If AT-SPI is unavailable
//! (e.g. no accessibility bus), fall back to command-line injectors:
//!   1. `xdotool type`  — X11; types arbitrary Unicode (incl. CJK) via XTEST
//!      keysym injection.
//!   2. `wtype`         — Wayland virtual keyboard.
//! If all fail, return an error so the phone shows "send failed" rather than
//! silently falling back to the clipboard (which the product forbids).

use std::process::Command;
use zbus::blocking::Connection;

const ATSPI_REGISTRY: &str = "org.a11y.atspi.Registry";
const ATSPI_ROOT: &str = "/org/a11y/atspi/accessible/root";
const STATE_ACTIVE: u32 = 1;
const STATE_FOCUSED: u32 = 12;

pub fn inject_text(text: &str) -> Result<(), String> {
    let atspi_err = match inject_atspi(text) {
        Ok(()) => {
            tracing::info!("aerodesk inject: AT-SPI ok ({})", text.chars().count());
            return Ok(());
        }
        Err(e) => {
            tracing::warn!("aerodesk inject: AT-SPI failed: {e}");
            e
        }
    };
    match inject_cli(text) {
        Ok(()) => {
            tracing::info!("aerodesk inject: CLI ok");
            Ok(())
        }
        Err(cli_err) => {
            tracing::warn!("aerodesk inject: CLI failed: {cli_err}");
            Err(format!("AT-SPI: {atspi_err}; CLI: {cli_err}"))
        }
    }
}

fn has_state(state: &[u32], bit: u32) -> bool {
    let idx = (bit / 32) as usize;
    state
        .get(idx)
        .map_or(false, |mask| mask & (1u32 << (bit % 32)) != 0)
}

fn has_editable_text(conn: &Connection, bus: &str, path: &str) -> bool {
    let ifaces: Vec<String> = conn
        .call_method(
            Some(bus),
            path,
            Some("org.a11y.atspi.Accessible"),
            "GetInterfaces",
            &(),
        )
        .map(|r| r.body().deserialize::<Vec<String>>().unwrap_or_default())
        .unwrap_or_default();
    ifaces.iter().any(|i| i.ends_with("EditableText"))
}

fn inject_atspi(text: &str) -> Result<(), String> {
    let session = Connection::session().map_err(|e| format!("AT-SPI unavailable: {e}"))?;
    let addr: String = session
        .call_method(
            Some("org.a11y.Bus"),
            "/org/a11y/bus",
            Some("org.a11y.Bus"),
            "GetAddress",
            &(),
        )
        .map_err(|e| format!("AT-SPI unavailable: {e}"))?
        .body()
        .deserialize()
        .map_err(|e| format!("AT-SPI unavailable: {e}"))?;
    let conn = zbus::blocking::connection::Builder::address(addr.as_str())
        .map_err(|e| format!("AT-SPI unavailable: {e}"))?
        .build()
        .map_err(|e| format!("AT-SPI unavailable: {e}"))?;
    let mut tried: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut v = std::collections::HashSet::new();
    if let Some(active) = find_first_with_state(&conn, ATSPI_REGISTRY, ATSPI_ROOT, STATE_ACTIVE, &mut v)? {
        let mut list = Vec::new();
        let mut seen = std::collections::HashSet::new();
        collect_focused_editables(&conn, active.0.as_str(), active.1.as_str(), &mut seen, &mut list)?;
        for f in &list {
            tried.insert(f.clone());
            if try_insert_verify(&conn, f, text)? {
                return Ok(());
            }
        }
    }
    let mut list2 = Vec::new();
    let mut seen2 = std::collections::HashSet::new();
    collect_focused_editables(&conn, ATSPI_REGISTRY, ATSPI_ROOT, &mut seen2, &mut list2)?;
    for f in &list2 {
        if !tried.insert(f.clone()) {
            continue;
        }
        if try_insert_verify(&conn, f, text)? {
            return Ok(());
        }
    }
    Err("no focused editable control accepted the text; click into a text field first".to_string())
}

fn accessible_name(conn: &Connection, focused: &(String, String)) -> Result<String, String> {
    let value: zbus::zvariant::OwnedValue = conn
        .call_method(
            Some(focused.0.as_str()),
            focused.1.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.a11y.atspi.Accessible", "Name"),
        )
        .map_err(|e| e.to_string())?
        .body()
        .deserialize()
        .map_err(|e| e.to_string())?;
    value
        .downcast_ref::<String>()
        .map(|s| s.to_string())
        .map_err(|e| e.to_string())
}

fn get_state(conn: &Connection, bus: &str, path: &str) -> Vec<u32> {
    conn.call_method(
        Some(bus),
        path,
        Some("org.a11y.atspi.Accessible"),
        "GetState",
        &(),
    )
    .map(|r| r.body().deserialize::<Vec<u32>>().unwrap_or_default())
    .unwrap_or_default()
}

fn get_children(
    conn: &Connection,
    bus: &str,
    path: &str,
) -> Vec<(String, zbus::zvariant::OwnedObjectPath)> {
    conn.call_method(
        Some(bus),
        path,
        Some("org.a11y.atspi.Accessible"),
        "GetChildren",
        &(),
    )
    .map(|r| {
        r.body()
            .deserialize::<Vec<(String, zbus::zvariant::OwnedObjectPath)>>()
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn find_first_with_state(
    conn: &Connection,
    bus: &str,
    path: &str,
    bit: u32,
    visited: &mut std::collections::HashSet<(String, String)>,
) -> Result<Option<(String, String)>, String> {
    if !visited.insert((bus.to_string(), path.to_string())) {
        return Ok(None);
    }
    if has_state(&get_state(conn, bus, path), bit) {
        return Ok(Some((bus.to_string(), path.to_string())));
    }
    for (cb, cp) in get_children(conn, bus, path) {
        if let Some(f) = find_first_with_state(conn, cb.as_str(), cp.as_str(), bit, visited)? {
            return Ok(Some(f));
        }
    }
    Ok(None)
}

fn collect_focused_editables(
    conn: &Connection,
    bus: &str,
    path: &str,
    visited: &mut std::collections::HashSet<(String, String)>,
    out: &mut Vec<(String, String)>,
) -> Result<(), String> {
    if !visited.insert((bus.to_string(), path.to_string())) {
        return Ok(());
    }
    if has_state(&get_state(conn, bus, path), STATE_FOCUSED) && has_editable_text(conn, bus, path) {
        out.push((bus.to_string(), path.to_string()));
    }
    for (cb, cp) in get_children(conn, bus, path) {
        collect_focused_editables(conn, cb.as_str(), cp.as_str(), visited, out)?;
    }
    Ok(())
}

fn get_caret_offset(conn: &Connection, target: &(String, String)) -> Result<i32, String> {
    let value: zbus::zvariant::OwnedValue = conn
        .call_method(
            Some(target.0.as_str()),
            target.1.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.a11y.atspi.Text", "CaretOffset"),
        )
        .map_err(|e| e.to_string())?
        .body()
        .deserialize()
        .map_err(|e| e.to_string())?;
    value.downcast_ref::<i32>().map_err(|e| e.to_string())
}

fn get_text_range(conn: &Connection, target: &(String, String), start: i32, end: i32) -> Option<String> {
    let s: String = conn
        .call_method(
            Some(target.0.as_str()),
            target.1.as_str(),
            Some("org.a11y.atspi.Text"),
            "GetText",
            &(start, end),
        )
        .ok()?
        .body()
        .deserialize()
        .ok()?;
    Some(s)
}

fn try_insert_verify(
    conn: &Connection,
    target: &(String, String),
    text: &str,
) -> Result<bool, String> {
    let caret = get_caret_offset(conn, target)?;
    if caret < 0 {
        return Ok(false);
    }
    let nchars = text.chars().count() as i32;
    let before = get_text_range(conn, target, caret, caret + nchars).unwrap_or_default();
    let ok: bool = conn
        .call_method(
            Some(target.0.as_str()),
            target.1.as_str(),
            Some("org.a11y.atspi.EditableText"),
            "InsertText",
            &(caret, text, text.len() as i32),
        )
        .map_err(|e| e.to_string())?
        .body()
        .deserialize()
        .map_err(|e| e.to_string())?;
    if !ok {
        return Ok(false);
    }
    let after = get_text_range(conn, target, caret, caret + nchars).unwrap_or_default();
    let verified = after == text || (after.contains(text) && !before.contains(text));
    let name = accessible_name(conn, target).unwrap_or_default();
    if verified {
        tracing::info!(
            "aerodesk inject: ok into {name} @ {} {} (caret={caret}, bytes={})",
            target.0,
            target.1,
            text.len()
        );
    } else {
        tracing::warn!(
            "aerodesk inject: {name} @ {} {} accepted InsertText but readback mismatch; skipping",
            target.0,
            target.1
        );
    }
    Ok(verified)
}

fn inject_cli(text: &str) -> Result<(), String> {
    let xdotool = Command::new("xdotool")
        .args(["type", "--clearmodifiers", "--delay", "0", "--"])
        .arg(text)
        .status();
    match xdotool {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("xdotool type failed (status {s})")),
        Err(_) => {
            let wtype = Command::new("wtype").arg(text).status();
            match wtype {
                Ok(s) if s.success() => Ok(()),
                Ok(s) => Err(format!("wtype failed (status {s})")),
                Err(_) => Err(
                    "no clipboard-free Linux injector found: install xdotool (X11) or wtype \
                     (Wayland), and enable the AT-SPI accessibility bus"
                        .to_string(),
                ),
            }
        }
    }
}
