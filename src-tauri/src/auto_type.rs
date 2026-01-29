#[cfg(target_os = "macos")]
mod macos {
    use std::time::Duration;

    // Minimal FFI surface so we don't need heavy wrappers.
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn CGEventCreateKeyboardEvent(
            source: *const std::ffi::c_void,
            virtualKey: u16,
            keyDown: bool,
        ) -> *mut std::ffi::c_void;
        fn CGEventKeyboardSetUnicodeString(
            event: *mut std::ffi::c_void,
            stringLength: usize,
            unicodeString: *const u16,
        );
        fn CGEventPost(tap: u32, event: *mut std::ffi::c_void);
        fn CFRelease(cf: *const std::ffi::c_void);
    }

    const K_CG_HID_EVENT_TAP: u32 = 0;

    fn post_unicode_chunk(chunk: &[u16]) {
        unsafe {
            // virtualKey is ignored when Unicode string is set.
            let down = CGEventCreateKeyboardEvent(std::ptr::null(), 0, true);
            if down.is_null() {
                return;
            }
            CGEventKeyboardSetUnicodeString(down, chunk.len(), chunk.as_ptr());
            CGEventPost(K_CG_HID_EVENT_TAP, down);
            CFRelease(down);

            let up = CGEventCreateKeyboardEvent(std::ptr::null(), 0, false);
            if up.is_null() {
                return;
            }
            CGEventKeyboardSetUnicodeString(up, chunk.len(), chunk.as_ptr());
            CGEventPost(K_CG_HID_EVENT_TAP, up);
            CFRelease(up);
        }
    }

    /// Types text into the currently focused app by posting CGEvents.
    ///
    /// Notes:
    /// - Uses unicode-string events (works for most apps/fields).
    /// - Converts '\n' to '\r' for better compatibility with terminals.
    /// - Chunks the unicode stream to reduce overhead.
    pub fn type_text(text: &str, per_chunk_delay: Duration) -> Result<(), String> {
        let mut sent_chunks: Vec<Vec<u16>> = Vec::new();
        if text.is_empty() {
            return Ok(());
        }

        // Normalize newlines for terminal / CLI friendliness.
        let normalized = text.replace('\n', "\r");
        let mut utf16: Vec<u16> = normalized.encode_utf16().collect();

        // Avoid huge single-event unicode strings.
        const CHUNK_U16: usize = 24;

        for chunk in utf16.chunks(CHUNK_U16) {
            if !sent_chunks.iter().any(|c| c == chunk) {
                post_unicode_chunk(chunk);
                sent_chunks.push(chunk.to_vec());
            }

            if per_chunk_delay.as_millis() > 0 {
                std::thread::sleep(per_chunk_delay);
            }
        }

        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub use macos::type_text;

#[cfg(not(target_os = "macos"))]
pub fn type_text(text: &str, _per_chunk_delay: std::time::Duration) -> Result<(), String> {
    // Non-macOS fallback: we keep the call site simple.
    // If you want Linux/Windows support later, implement here.
    let _ = text;
    Err("Auto-typing is only implemented on macOS in this MVP.".into())
}
