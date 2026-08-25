# AeroDesk

AeroDesk is the **cross-platform desktop companion** for Aerofont. The phone
records + polishes text; AeroDesk receives it over a **LAN WebSocket** and
**pastes it at the desktop cursor — without touching the clipboard**.

Runs on macOS and Windows. Tauri v2 + Rust.

## Pairing

1. Start AeroDesk — it shows a QR code encoding `{v,url,token,name}`.
2. In the Aerofont mobile app, unlock the hidden AeroDesk feature (tap the
   top-left logo 8 times), then scan the QR from Settings → AeroDesk.
3. Toggle the keyboard's "send to desktop" mode. Speak — the polished text lands
   at the desktop cursor instead of the phone's.

The token + port persist to `~/.config/aerodesk/config.json`
(`~/Library/Application Support/aerodesk/` on macOS, `%APPDATA%\aerodesk\` on
Windows), so restarts don't break pairing. If the desktop's LAN IP changes
(DHCP), re-scan.