//! macOS platform implementation.
//!
//! Uses:
//! - `AXIsProcessTrusted` for accessibility checks
//! - `CGEventTap` for Fn key listening
//! - `CGEventCreateKeyboardEvent` for text injection
//! - `cpal` for audio capture (cross-platform, but configured here)

use super::{KeyCallback, KeyListenerHandle, Platform, RecordingHandle};
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result as AnyhowResult};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

// ─────────────────────────────────────────────────────────────────────────────
// Platform Implementation
// ─────────────────────────────────────────────────────────────────────────────

pub struct MacOSPlatform;

impl Platform for MacOSPlatform {
    fn is_accessibility_trusted(&self) -> bool {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        unsafe { AXIsProcessTrusted() }
    }

    fn request_accessibility_permission(&self) -> bool {
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFDictionaryCreate(
                allocator: *const c_void,
                keys: *const *const c_void,
                values: *const *const c_void,
                numValues: isize,
                keyCallBacks: *const c_void,
                valueCallBacks: *const c_void,
            ) -> *const c_void;
            fn CFRelease(cf: *const c_void);
            static kCFBooleanTrue: *const c_void;
        }

        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
            static kAXTrustedCheckOptionPrompt: *const c_void;
        }

        unsafe {
            let keys = [kAXTrustedCheckOptionPrompt];
            let values = [kCFBooleanTrue];
            let options = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr() as *const *const c_void,
                values.as_ptr() as *const *const c_void,
                1,
                std::ptr::null(),
                std::ptr::null(),
            );

            let result = AXIsProcessTrustedWithOptions(options);

            if !options.is_null() {
                CFRelease(options);
            }

            result
        }
    }

    fn start_fn_key_listener(
        &self,
        callback: KeyCallback,
    ) -> Result<Box<dyn KeyListenerHandle>, String> {
        let listener = FnKeyListenerImpl::new(callback)?;
        Ok(Box::new(listener))
    }

    fn type_text(&self, text: &str, per_chunk_delay: Duration) -> Result<(), String> {
        type_text_impl(text, per_chunk_delay)
    }

    fn start_audio_capture(&self) -> Result<Box<dyn RecordingHandle>, String> {
        let session = RecordingSession::start().map_err(|e| e.to_string())?;
        Ok(Box::new(session))
    }

    fn name(&self) -> &'static str {
        "macos"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fn Key Listener (CGEventTap)
// ─────────────────────────────────────────────────────────────────────────────

#[repr(C)]
#[allow(non_camel_case_types)]
struct CFRunLoopSource {
    _opaque: [u8; 0],
}
#[repr(C)]
#[allow(non_camel_case_types)]
struct CFMachPort {
    _opaque: [u8; 0],
}
#[repr(C)]
#[allow(non_camel_case_types)]
struct CFRunLoop {
    _opaque: [u8; 0],
}
#[repr(C)]
#[allow(non_camel_case_types)]
struct CGEvent {
    _opaque: [u8; 0],
}

type CGEventTapCallBack = extern "C" fn(
    proxy: *mut c_void,
    event_type: u32,
    event: *mut CGEvent,
    user_info: *mut c_void,
) -> *mut CGEvent;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> *mut CFMachPort;

    fn CGEventTapEnable(tap: *mut CFMachPort, enable: bool);

    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        tap: *mut CFMachPort,
        order: isize,
    ) -> *mut CFRunLoopSource;

    fn CFRunLoopGetCurrent() -> *mut CFRunLoop;
    fn CFRunLoopAddSource(rl: *mut CFRunLoop, source: *mut CFRunLoopSource, mode: *const c_void);

    fn CGEventGetFlags(event: *const CGEvent) -> u64;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFRunLoopCommonModes: *const c_void;
}

// CGEventMask for flagsChanged
const K_CG_EVENT_FLAGS_CHANGED: u32 = 12;
const K_CG_EVENT_MASK_FLAGS_CHANGED: u64 = 1 << K_CG_EVENT_FLAGS_CHANGED;

// CGEventTapLocation
const K_CG_HID_EVENT_TAP: u32 = 0;
// CGEventTapPlacement
const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
// CGEventTapOptions
const K_CG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;

// CGEventFlags - Fn key flag
const K_CG_EVENT_FLAG_MASK_SECONDARY_FN: u64 = 0x800000;

pub struct FnKeyListenerImpl {
    tap: *mut CFMachPort,
    #[allow(dead_code)]
    callback: Arc<Mutex<KeyCallback>>,
}

unsafe impl Send for FnKeyListenerImpl {}
unsafe impl Sync for FnKeyListenerImpl {}

extern "C" fn event_tap_callback(
    _proxy: *mut c_void,
    event_type: u32,
    event: *mut CGEvent,
    user_info: *mut c_void,
) -> *mut CGEvent {
    unsafe {
        if event_type == K_CG_EVENT_FLAGS_CHANGED {
            let flags = CGEventGetFlags(event);
            let fn_pressed = (flags & K_CG_EVENT_FLAG_MASK_SECONDARY_FN) != 0;

            if !user_info.is_null() {
                let callback_ptr = user_info as *mut Arc<Mutex<KeyCallback>>;
                if let Some(callback_arc) = callback_ptr.as_ref() {
                    if let Ok(mut cb) = callback_arc.lock() {
                        cb(fn_pressed);
                    }
                }
            }
        }
        event
    }
}

impl FnKeyListenerImpl {
    pub fn new(callback: KeyCallback) -> Result<Self, String> {
        unsafe {
            let callback_arc = Arc::new(Mutex::new(callback));
            let user_info = Box::into_raw(Box::new(callback_arc.clone())) as *mut c_void;

            let tap = CGEventTapCreate(
                K_CG_HID_EVENT_TAP,
                K_CG_HEAD_INSERT_EVENT_TAP,
                K_CG_EVENT_TAP_OPTION_DEFAULT,
                K_CG_EVENT_MASK_FLAGS_CHANGED,
                event_tap_callback,
                user_info,
            );

            if tap.is_null() {
                // Clean up user_info if tap creation failed
                let _ = Box::from_raw(user_info);
                return Err("Failed to create event tap. Please:\n\
                    1. Completely quit this app (Cmd+Q)\n\
                    2. Reopen it\n\
                    3. Try enabling Fn key again\n\n\
                    Note: macOS requires app restart after granting Accessibility permission."
                    .into());
            }

            let run_loop_source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
            if run_loop_source.is_null() {
                return Err("Failed to create run loop source".into());
            }

            let run_loop = CFRunLoopGetCurrent();
            CFRunLoopAddSource(run_loop, run_loop_source, kCFRunLoopCommonModes);

            CGEventTapEnable(tap, true);

            Ok(FnKeyListenerImpl {
                tap,
                callback: callback_arc,
            })
        }
    }
}

impl KeyListenerHandle for FnKeyListenerImpl {
    fn stop(&self) {
        unsafe {
            if !self.tap.is_null() {
                CGEventTapEnable(self.tap, false);
            }
        }
    }
}

impl Drop for FnKeyListenerImpl {
    fn drop(&mut self) {
        self.stop();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Text Injection (CGEvent keyboard events)
// ─────────────────────────────────────────────────────────────────────────────

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventCreateKeyboardEvent(
        source: *const c_void,
        virtualKey: u16,
        keyDown: bool,
    ) -> *mut c_void;
    fn CGEventKeyboardSetUnicodeString(
        event: *mut c_void,
        stringLength: usize,
        unicodeString: *const u16,
    );
    fn CGEventPost(tap: u32, event: *mut c_void);
    fn CFRelease(cf: *const c_void);
}

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
fn type_text_impl(text: &str, per_chunk_delay: Duration) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }

    // Normalize newlines for terminal / CLI friendliness.
    let normalized = text.replace('\n', "\r");
    let utf16: Vec<u16> = normalized.encode_utf16().collect();

    // Avoid huge single-event unicode strings.
    const CHUNK_U16: usize = 24;

    for chunk in utf16.chunks(CHUNK_U16) {
        post_unicode_chunk(chunk);

        if per_chunk_delay.as_millis() > 0 {
            std::thread::sleep(per_chunk_delay);
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Audio Capture (cpal-based, same as before)
// ─────────────────────────────────────────────────────────────────────────────

/// A Send handle for an in-progress recording.
///
/// The actual `cpal::Stream` is created and owned inside a dedicated thread
/// because `cpal::Stream` is not `Send`/`Sync` on macOS.
struct RecordingSession {
    stop_tx: mpsc::Sender<()>,
    done_rx: mpsc::Receiver<AnyhowResult<PathBuf>>,
}

impl RecordingSession {
    fn start() -> AnyhowResult<Self> {
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (done_tx, done_rx) = mpsc::channel::<AnyhowResult<PathBuf>>();

        thread::spawn(move || {
            let res = (|| -> AnyhowResult<PathBuf> {
                let host = cpal::default_host();
                let device = host
                    .default_input_device()
                    .ok_or_else(|| anyhow!("No default input device"))?;

                let supported_config = device
                    .default_input_config()
                    .context("Failed to get default input config")?;

                let sample_rate = supported_config.sample_rate().0;
                let channels = supported_config.channels();

                let samples: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::new()));
                let samples_cb = samples.clone();

                let err_fn = |err| eprintln!("an error occurred on the input audio stream: {err}");

                let stream = match supported_config.sample_format() {
                    cpal::SampleFormat::I16 => {
                        let config: cpal::StreamConfig = supported_config.into();
                        device.build_input_stream(
                            &config,
                            move |data: &[i16], _| {
                                if let Ok(mut buf) = samples_cb.lock() {
                                    buf.extend_from_slice(data);
                                }
                            },
                            err_fn,
                            None,
                        )?
                    }
                    cpal::SampleFormat::U16 => {
                        let config: cpal::StreamConfig = supported_config.into();
                        device.build_input_stream(
                            &config,
                            move |data: &[u16], _| {
                                if let Ok(mut buf) = samples_cb.lock() {
                                    for &s in data {
                                        let v: i16 = (s as i32 - 32768) as i16;
                                        buf.push(v);
                                    }
                                }
                            },
                            err_fn,
                            None,
                        )?
                    }
                    cpal::SampleFormat::F32 => {
                        let config: cpal::StreamConfig = supported_config.into();
                        device.build_input_stream(
                            &config,
                            move |data: &[f32], _| {
                                if let Ok(mut buf) = samples_cb.lock() {
                                    for &s in data {
                                        let v: i16 = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                                        buf.push(v);
                                    }
                                }
                            },
                            err_fn,
                            None,
                        )?
                    }
                    other => return Err(anyhow!("Unsupported sample format: {other:?}")),
                };

                stream.play()?;

                // Block until stop signal.
                let _ = stop_rx.recv();
                drop(stream);

                // Write WAV.
                let samples = samples
                    .lock()
                    .map_err(|_| anyhow!("Failed to lock samples"))?;

                let mut path = std::env::temp_dir();
                let filename = format!(
                    "groqtranscriber-{}.wav",
                    chrono::Utc::now().format("%Y%m%d-%H%M%S")
                );
                path.push(filename);

                let spec = hound::WavSpec {
                    channels,
                    sample_rate,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                };

                let mut writer =
                    hound::WavWriter::create(&path, spec).context("Failed to create wav")?;
                for &s in samples.iter() {
                    writer.write_sample(s).ok();
                }
                writer.finalize().ok();

                Ok(path)
            })();

            let _ = done_tx.send(res);
        });

        Ok(Self { stop_tx, done_rx })
    }
}

impl RecordingHandle for RecordingSession {
    fn stop_and_save_wav(self: Box<Self>) -> Result<PathBuf, String> {
        let _ = self.stop_tx.send(());
        self.done_rx
            .recv()
            .map_err(|_| "Recording thread terminated unexpectedly".to_string())?
            .map_err(|e| e.to_string())
    }
}
