#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

/// Insert `text` at the current cursor in whatever app has focus, WITHOUT
/// touching the clipboard.
pub fn inject_text(text: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        macos::inject_text(text)
    }
    #[cfg(target_os = "windows")]
    {
        windows::inject_text(text)
    }
    #[cfg(target_os = "linux")]
    {
        linux::inject_text(text)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = text;
        Err("unsupported platform".to_string())
    }
}

/// macOS-only: whether the process has Accessibility (TCC) permission, which
/// CGEvent-based injection requires. Returns `true` everywhere else.
pub fn is_accessibility_trusted() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::is_accessibility_trusted()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}