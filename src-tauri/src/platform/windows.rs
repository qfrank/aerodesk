//! Windows: clipboard-free Unicode injection via SendInput + KEYEVENTF_UNICODE
//! (enigo's `text()`). Bypasses the keyboard layout and the IME — Chinese lands
//! directly at the cursor. No special permission required.

use enigo::{Enigo, Keyboard, Settings};

pub fn inject_text(text: &str) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.text(text).map_err(|e| e.to_string())
}