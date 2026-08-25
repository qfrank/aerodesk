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
Windows), so restarts don't break pairing. The LAN IP is re-detected on every
launch (a host you saved manually wins until you save a different one); if
pairing still fails, re-scan. The Windows installer adds a Windows Firewall
inbound allow rule for `AeroDesk.exe` automatically — if you run it some other
way, allow it inbound or the phone's connection is silently dropped.