//! Windows platform implementation.
//!
//! This provides a pragmatic Windows implementation for the GroqBara
//! (GroqTranscriber) core features:
//!
//! - Accessibility checks are essentially no-ops (`true`), since Windows
//!   does not gate keyboard hooks / SendInput behind a user-facing prompt
//!   like macOS Accessibility.
//! - `start_fn_key_listener` is implemented via a low-level keyboard hook
//!   (`WH_KEYBOARD_LL`) that watches a *configurable* trigger key
//!   (default: Right Alt; F24 is supported as an alternative).
//! - `type_text` uses `SendInput` with `KEYEVENTF_UNICODE` so we can inject
//!   arbitrary Unicode text into the focused app.
//! - `start_audio_capture` reuses the same `cpal`-based WAV recording
//!   pipeline as macOS.

use super::{KeyCallback, KeyListenerHandle, Platform, RecordingHandle};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result as AnyhowResult};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP, VK_F24, VK_RMENU,
};

// ─────────────────────────────────────────────────────────────────────────────
// Platform Implementation
// ─────────────────────────────────────────────────────────────────────────────

pub struct WindowsPlatform;

impl Platform for WindowsPlatform {
    fn is_accessibility_trusted(&self) -> bool {
        // Windows does not expose a macOS-style Accessibility permission gate
        // for low-level input hooks / SendInput in classic desktop apps.
        // If the process is running, we assume we can operate.
        true
    }

    fn request_accessibility_permission(&self) -> bool {
        // No user-facing Accessibility prompt on Windows. If the app is
        // able to install hooks and call SendInput, we're good.
        true
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
        "windows"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fn Key Listener (WH_KEYBOARD_LL)
// ─────────────────────────────────────────────────────────────────────────────

/// Default trigger key on Windows.
///
/// We treat the **Right Alt** key (VK_RMENU) as the "Fn-equivalent" trigger,
/// because real Fn keys are typically handled at the firmware/driver level and
/// never reach Windows as keyboard events.
///
/// F24 is supported as an alternative "extra" function key that many users
/// bind via tools like AutoHotkey.
const DEFAULT_TRIGGER_VKS: [u32; 2] = [VK_RMENU.0 as u32, VK_F24.0 as u32];

/// Global storage for the callback used by the low-level hook.
static KEY_CALLBACK: OnceLock<Arc<Mutex<KeyCallback>>> = OnceLock::new();

extern "system" fn keyboard_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    unsafe {
        if n_code == HC_ACTION {
            let kb = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
            let vk = kb.vkCode;

            if DEFAULT_TRIGGER_VKS.contains(&vk) {
                let msg = w_param.0 as u32;
                let pressed = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                let released = msg == WM_KEYUP || msg == WM_SYSKEYUP;

                if pressed || released {
                    if let Some(cb_arc) = KEY_CALLBACK.get() {
                        if let Ok(mut cb) = cb_arc.lock() {
                            cb(pressed);
                        }
                    }
                }
            }
        }

        CallNextHookEx(HHOOK(0), n_code, w_param, l_param)
    }
}

pub struct FnKeyListenerImpl {
    hook: HHOOK,
    thread_id: u32,
}

impl FnKeyListenerImpl {
    pub fn new(callback: KeyCallback) -> Result<Self, String> {
        // Store callback in global OnceLock (single listener instance).
        let cb_arc = Arc::new(Mutex::new(callback));
        if KEY_CALLBACK.set(cb_arc).is_err() {
            return Err("Fn key listener already active".into());
        }

        let (tx, rx) = mpsc::channel::<Result<(HHOOK, u32), String>>();

        thread::spawn(move || {
            unsafe {
                // Install low-level keyboard hook on this thread.
                let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0);
                if hook.is_invalid() {
                    let _ = tx.send(Err("SetWindowsHookExW(WH_KEYBOARD_LL) failed".into()));
                    return;
                }

                // This thread ID is used by `stop()` to post WM_QUIT.
                let thread_id = windows::Win32::System::Threading::GetCurrentThreadId();

                if tx.send(Ok((hook, thread_id))).is_err() {
                    // Creator went away; just unhook and exit.
                    let _ = windows::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(hook);
                    return;
                }

                // Simple message loop to keep WH_KEYBOARD_LL alive.
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, HWND(0), 0, 0).into() {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                let _ = windows::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(hook);
            }
        });

        let (hook, thread_id) = rx
            .recv()
            .map_err(|_| "Keyboard hook thread failed to start".to_string())??;

        Ok(Self { hook, thread_id })
    }
}

impl KeyListenerHandle for FnKeyListenerImpl {
    fn stop(&self) {
        unsafe {
            // Ask the hook thread to exit its message loop.
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            // Actual UnhookWindowsHookEx is done in the hook thread on exit.

            // We intentionally do **not** reset KEY_CALLBACK here, so that any
            // in-flight callbacks don't see a suddenly-missing closure. The
            // process is short-lived and only one listener is supported.
            let _ = self.hook; // suppress unused warning (field kept for future use/debug).
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Text Injection (SendInput + KEYEVENTF_UNICODE)
// ─────────────────────────────────────────────────────────────────────────────

fn type_text_impl(text: &str, per_chunk_delay: Duration) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }

    // Normalize newlines for Windows: use CRLF.
    let normalized = text.replace('\n', "\r\n");
    let utf16: Vec<u16> = normalized.encode_utf16().collect();

    // Chunk to avoid huge single SendInput calls.
    const CHUNK_U16: usize = 24;

    for chunk in utf16.chunks(CHUNK_U16) {
        for &unit in chunk {
            unsafe {
                let inputs = [
                    INPUT {
                        r#type: INPUT_KEYBOARD,
                        Anonymous: INPUT_0 {
                            ki: KEYBDINPUT {
                                wVk: 0,
                                wScan: unit,
                                dwFlags: KEYEVENTF_UNICODE,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    },
                    INPUT {
                        r#type: INPUT_KEYBOARD,
                        Anonymous: INPUT_0 {
                            ki: KEYBDINPUT {
                                wVk: 0,
                                wScan: unit,
                                dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                                time: 0,
                                dwExtraInfo: 0,
                            },
                        },
                    },
                ];

                let sent = SendInput(
                    inputs.len() as u32,
                    inputs.as_ptr(),
                    std::mem::size_of::<INPUT>() as i32,
                );

                if sent == 0 {
                    return Err("SendInput failed while typing".into());
                }
            }
        }

        if per_chunk_delay.as_millis() > 0 {
            std::thread::sleep(per_chunk_delay);
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Audio Capture (cpal-based, same pipeline as macOS)
// ─────────────────────────────────────────────────────────────────────────────

/// A Send handle for an in-progress recording.
///
/// The actual `cpal::Stream` is created and owned inside a dedicated thread
/// because `cpal::Stream` is not `Send`/`Sync` on all platforms.
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
