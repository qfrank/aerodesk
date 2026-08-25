//! macOS: clipboard-free Unicode injection via CGEvent unicode string events
//! (enigo's `text()`). Bypasses the IME — Chinese lands directly at the cursor.
//! Requires a one-time Accessibility (TCC) grant; see `is_accessibility_trusted`.

use enigo::{Enigo, Keyboard, Settings};

pub fn inject_text(text: &str) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.text(text).map_err(|e| e.to_string())
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

pub fn is_accessibility_trusted() -> bool {
    // SAFETY: read-only C function, no aliasing concerns.
    unsafe { AXIsProcessTrusted() }
}