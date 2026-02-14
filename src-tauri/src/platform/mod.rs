//! Platform abstraction layer for GroqBara (ito-but-free).
//!
//! This module provides cross-platform abstractions for OS-specific functionality:
//! - **Hotkey registration** (global shortcuts beyond what tauri-plugin-global-shortcut provides)
//! - **Key hold/toggle trigger detection** (e.g., Fn key on macOS)
//! - **Audio capture** (microphone recording)
//! - **Text injection** (auto-type into focused applications)
//!
//! # Architecture
//!
//! The [`Platform`] trait defines the interface. Platform-specific implementations
//! live in submodules (`macos`, `windows`), selected at compile time via `#[cfg]`.
//!
//! The [`current()`] function returns a boxed trait object for the current platform.

use std::path::PathBuf;
use std::time::Duration;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

// Re-export the FnKeyListener for compatibility with existing code
#[cfg(target_os = "macos")]
pub use macos::FnKeyListenerImpl;

#[cfg(target_os = "windows")]
pub use windows::FnKeyListenerImpl;

/// Callback type for key state changes (pressed/released).
pub type KeyCallback = Box<dyn FnMut(bool) + Send + 'static>;

/// Handle for an active key listener that can be stopped.
pub trait KeyListenerHandle: Send + Sync {
    /// Stop listening for key events.
    fn stop(&self);
}

/// Handle for an active audio recording session.
pub trait RecordingHandle: Send {
    /// Stop recording and save to WAV file.
    /// Returns the path to the saved WAV file.
    fn stop_and_save_wav(self: Box<Self>) -> Result<PathBuf, String>;
}

/// Platform abstraction trait.
///
/// Implementations provide OS-specific functionality for:
/// - Accessibility permission checks
/// - Fn-key (or equivalent) listening
/// - Text injection (auto-type)
/// - Audio capture
///
/// # Example
///
/// ```ignore
/// let platform = platform::current();
/// if !platform.is_accessibility_trusted() {
///     platform.request_accessibility_permission();
/// }
/// ```
pub trait Platform: Send + Sync {
    // ─────────────────────────────────────────────────────────────────────────
    // Accessibility
    // ─────────────────────────────────────────────────────────────────────────

    /// Check if the app has accessibility permissions (required for key listening
    /// and text injection on macOS).
    fn is_accessibility_trusted(&self) -> bool;

    /// Request accessibility permission from the user.
    /// On macOS, this shows the system prompt. On Windows, this may be a no-op.
    /// Returns true if permission is already granted.
    fn request_accessibility_permission(&self) -> bool;

    // ─────────────────────────────────────────────────────────────────────────
    // Key Listening (Fn key / special key triggers)
    // ─────────────────────────────────────────────────────────────────────────

    /// Start listening for Fn key (or platform-equivalent) press/release events.
    ///
    /// The callback receives `true` when the key is pressed, `false` when released.
    ///
    /// Returns a handle that can be used to stop listening.
    fn start_fn_key_listener(
        &self,
        callback: KeyCallback,
    ) -> Result<Box<dyn KeyListenerHandle>, String>;

    // ─────────────────────────────────────────────────────────────────────────
    // Text Injection (Auto-type)
    // ─────────────────────────────────────────────────────────────────────────

    /// Type text into the currently focused application.
    ///
    /// This simulates keyboard input to insert text character-by-character.
    /// Requires accessibility permissions on macOS.
    ///
    /// # Arguments
    /// * `text` - The text to type
    /// * `per_chunk_delay` - Delay between chunks of characters (for rate limiting)
    fn type_text(&self, text: &str, per_chunk_delay: Duration) -> Result<(), String>;

    // ─────────────────────────────────────────────────────────────────────────
    // Audio Capture
    // ─────────────────────────────────────────────────────────────────────────

    /// Start capturing audio from the default input device.
    ///
    /// `gain` is a multiplier applied to samples (1.0 = no change, >1.0 = louder).
    /// Returns a handle that can be used to stop recording and save to WAV.
    fn start_audio_capture(&self, gain: f32) -> Result<Box<dyn RecordingHandle>, String>;

    // ─────────────────────────────────────────────────────────────────────────
    // Platform Info
    // ─────────────────────────────────────────────────────────────────────────

    /// Return the platform name (e.g., "macos", "windows").
    fn name(&self) -> &'static str;
}

/// Get the platform implementation for the current OS.
///
/// This is the main entry point for platform-specific functionality.
///
/// # Example
///
/// ```ignore
/// let platform = platform::current();
/// println!("Running on: {}", platform.name());
/// ```
#[cfg(target_os = "macos")]
pub fn current() -> Box<dyn Platform> {
    Box::new(macos::MacOSPlatform)
}

#[cfg(target_os = "windows")]
pub fn current() -> Box<dyn Platform> {
    Box::new(windows::WindowsPlatform)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn current() -> Box<dyn Platform> {
    compile_error!("Unsupported platform. Only macOS and Windows are supported.");
}

// ─────────────────────────────────────────────────────────────────────────────
// Compatibility shim for FnKeyListener
// ─────────────────────────────────────────────────────────────────────────────

/// FnKeyListener compatibility wrapper.
///
/// This maintains API compatibility with the existing `fn_key_listener` module
/// while using the new platform abstraction internally.
pub struct FnKeyListener {
    handle: Box<dyn KeyListenerHandle>,
}

impl FnKeyListener {
    /// Create a new Fn key listener.
    ///
    /// The callback receives `true` when Fn is pressed, `false` when released.
    pub fn new<F>(callback: F) -> Result<Self, String>
    where
        F: FnMut(bool) + Send + 'static,
    {
        let platform = current();
        let handle = platform.start_fn_key_listener(Box::new(callback))?;
        Ok(Self { handle })
    }

    /// Stop listening for Fn key events.
    pub fn stop(&self) {
        self.handle.stop();
    }
}
