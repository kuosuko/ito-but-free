use arboard::Clipboard;
use serde::Serialize;
use std::{str::FromStr, time::Duration};
use tauri::{
    AppHandle, Emitter, Manager, Runtime,
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub mod platform;
mod settings;
mod transcribe;

/// Wrapper for the platform FnKeyListener to implement required traits.
struct FnKeyListenerWrapper(platform::FnKeyListener);

// The wrapper is Send + Sync because platform::FnKeyListener's handle is.
unsafe impl Send for FnKeyListenerWrapper {}
unsafe impl Sync for FnKeyListenerWrapper {}

impl FnKeyListenerWrapper {
    fn stop(&self) {
        self.0.stop();
    }
}

struct AppState {
    session: std::sync::Mutex<Option<Box<dyn platform::RecordingHandle>>>,
    hotkey: std::sync::Mutex<Option<Shortcut>>,
    fn_listener: std::sync::Mutex<Option<FnKeyListenerWrapper>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            session: std::sync::Mutex::new(None),
            hotkey: std::sync::Mutex::new(None),
            fn_listener: std::sync::Mutex::new(None),
        }
    }
}

#[derive(Serialize, Clone)]
struct LogEvent {
    level: String,
    message: String,
}

fn emit_log<R: Runtime>(app: &AppHandle<R>, level: &str, message: impl Into<String>) {
    let _ = app.emit(
        "log",
        LogEvent {
            level: level.to_string(),
            message: message.into(),
        },
    );
}

// Default shortcut (platform-specific):
// macOS: F13 — common "extra" function key on Mac keyboards.
// Windows: Ctrl+Space — F13 doesn't exist on most PC keyboards.
#[cfg(target_os = "macos")]
const DEFAULT_HOTKEY: &str = "F13";
#[cfg(target_os = "windows")]
const DEFAULT_HOTKEY: &str = "Control+Space";

const DEFAULT_TRIGGER_MODE: &str = "hold";

const DEFAULT_AUTO_TYPE_ENABLED: bool = true;
const DEFAULT_TYPE_SPEED_MS: u64 = 0;

fn resolve_auto_type_enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    settings::get_auto_type_enabled(app)
        .ok()
        .flatten()
        .unwrap_or(DEFAULT_AUTO_TYPE_ENABLED)
}

fn resolve_type_speed_ms<R: Runtime>(app: &AppHandle<R>) -> u64 {
    settings::get_type_speed_ms(app)
        .ok()
        .flatten()
        .unwrap_or(DEFAULT_TYPE_SPEED_MS)
}

const DEFAULT_MIC_GAIN: f32 = 1.0;

fn resolve_mic_gain<R: Runtime>(app: &AppHandle<R>) -> f32 {
    settings::get_mic_gain(app)
        .ok()
        .flatten()
        .unwrap_or(DEFAULT_MIC_GAIN)
}

fn resolve_trigger_mode<R: Runtime>(app: &AppHandle<R>) -> String {
    settings::get_trigger_mode(app)
        .ok()
        .flatten()
        .unwrap_or_else(|| DEFAULT_TRIGGER_MODE.to_string())
}

/// Check if accessibility permissions are granted (uses platform abstraction).
fn is_accessibility_trusted() -> bool {
    platform::current().is_accessibility_trusted()
}

/// Request accessibility permission (uses platform abstraction).
#[allow(dead_code)]
fn request_accessibility_permission() -> bool {
    platform::current().request_accessibility_permission()
}

// (clipboard removed)

fn set_clipboard_text(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| format!("Clipboard init failed: {e}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|e| format!("Failed to set clipboard: {e}"))?;
    Ok(())
}

// Auto-typing implementation lives in src-tauri/src/auto_type.rs

fn type_text_into_focused_app<R: Runtime>(app: &AppHandle<R>, text: &str) -> Result<(), String> {
    if text.trim().is_empty() {
        return Ok(());
    }

    if !is_accessibility_trusted() {
        return Err(
            "Accessibility permission is required to type into other apps. Enable it in System Settings → Privacy & Security → Accessibility.".into(),
        );
    }

    let delay_ms = resolve_type_speed_ms(app);
    let delay = Duration::from_millis(delay_ms);
    
    // Use platform abstraction for text injection
    platform::current().type_text(text, delay)
}

fn resolve_hotkey_string<R: Runtime>(app: &AppHandle<R>) -> String {
    settings::get_global_hotkey(app)
        .ok()
        .flatten()
        .unwrap_or_else(|| DEFAULT_HOTKEY.to_string())
}

fn register_hotkey<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    hotkey_str: &str,
) -> Result<(), String> {
    let shortcut = Shortcut::from_str(hotkey_str)
        .map_err(|e| format!("Invalid hotkey '{hotkey_str}': {e}"))?;

    // Unregister previous hotkey (best-effort).
    let mut guard = state.hotkey.lock().map_err(|e| e.to_string())?;
    if let Some(prev) = guard.take() {
        let _ = app.global_shortcut().unregister(prev);
    }

    let app_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            let app = app_handle.clone();

            // Capture trigger mode at event time (settings can change at runtime).
            let mode = resolve_trigger_mode(&app);

            match mode.as_str() {
                "toggle" => {
                    // The underlying hotkey library fires on both key press and key release.
                    // For toggle, only react to the initial key press.
                    if event.state != ShortcutState::Pressed {
                        return;
                    }

                    tauri::async_runtime::spawn(async move {
                        let state = app.state::<AppState>();
                        let is_recording = state
                            .inner()
                            .session
                            .lock()
                            .ok()
                            .map(|g| g.is_some())
                            .unwrap_or(false);

                        if !is_recording {
                            if let Err(e) = do_start_recording(&app, state.inner()) {
                                emit_log(&app, "error", format!("Failed to start recording: {e}"));
                            }
                            return;
                        }

                        match do_stop_and_transcribe(&app, state.inner()).await {
                            Ok(text) => {
                                let _ = app.emit("transcription", text.clone());

                                if resolve_auto_type_enabled(&app) {
                                    match type_text_into_focused_app(&app, &text) {
                                        Ok(()) => emit_log(
                                            &app,
                                            "info",
                                            "Auto-typed transcription into focused app",
                                        ),
                                        Err(e) => emit_log(
                                            &app,
                                            "error",
                                            format!("Auto-type failed: {e}"),
                                        ),
                                    }
                                }
                            }
                            Err(e) => {
                                emit_log(&app, "error", format!("Failed to stop/transcribe: {e}"))
                            }
                        }
                    });
                }
                _ => {
                    // hold (default)
                    match event.state {
                        ShortcutState::Pressed => {
                            let state = app.state::<AppState>();
                            if let Err(e) = do_start_recording(&app, state.inner()) {
                                // Ignore "Already recording" for a held key repeat.
                                if e != "Already recording" {
                                    emit_log(&app, "error", format!("Failed to start recording: {e}"));
                                }
                            }
                        }
                        ShortcutState::Released => {
                            tauri::async_runtime::spawn(async move {
                                let state = app.state::<AppState>();
                                let is_recording = state
                                    .inner()
                                    .session
                                    .lock()
                                    .ok()
                                    .map(|g| g.is_some())
                                    .unwrap_or(false);
                                if !is_recording {
                                    return;
                                }

                                match do_stop_and_transcribe(&app, state.inner()).await {
                                    Ok(text) => {
                                        let _ = app.emit("transcription", text.clone());

                                        if resolve_auto_type_enabled(&app) {
                                            match type_text_into_focused_app(&app, &text) {
                                                Ok(()) => emit_log(
                                                    &app,
                                                    "info",
                                                    "Auto-typed transcription into focused app",
                                                ),
                                                Err(e) => emit_log(
                                                    &app,
                                                    "error",
                                                    format!("Auto-type failed: {e}"),
                                                ),
                                            }
                                        }
                                    }
                                    Err(e) => emit_log(
                                        &app,
                                        "error",
                                        format!("Failed to stop/transcribe: {e}"),
                                    ),
                                }
                            });
                        }
                    }
                }
            }
        })
        .map_err(|e| format!("Failed to register global shortcut: {e}"))?;

    *guard = Some(shortcut);

    emit_log(
        app,
        "info",
        format!("Global shortcut registered: {hotkey_str}"),
    );
    Ok(())
}

fn do_start_recording<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<(), String> {
    let mut guard = state.session.lock().map_err(|e| e.to_string())?;
    if guard.is_some() {
        return Err("Already recording".into());
    }

    emit_log(app, "info", "Starting recording...");
    let gain = resolve_mic_gain(app);
    let session = platform::current().start_audio_capture(gain)?;
    *guard = Some(session);
    let _ = app.emit("recording_state", "recording");
    // Show floating overlay
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.show();
    }
    emit_log(app, "info", "Recording started");
    Ok(())
}

async fn do_stop_and_transcribe<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<String, String> {
    let session = {
        let mut guard = state.session.lock().map_err(|e| e.to_string())?;
        guard.take()
    };

    let Some(session) = session else {
        return Err("Not recording".into());
    };

    let _ = app.emit("recording_state", "processing");
    let result = do_transcription_pipeline(app, session).await;
    let _ = app.emit("recording_state", "idle");
    // Hide floating overlay
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.hide();
    }
    result
}

/// Inner pipeline: stop recording → transcribe → optionally refine.
/// Separated so `do_stop_and_transcribe` can always emit "idle" on completion.
async fn do_transcription_pipeline<R: Runtime>(
    app: &AppHandle<R>,
    session: Box<dyn platform::RecordingHandle>,
) -> Result<String, String> {
    emit_log(app, "info", "Stopping recording...");
    let wav_path = session.stop_and_save_wav()?;
    emit_log(app, "info", format!("Saved WAV: {}", wav_path.display()));

    // API key resolution: settings.json > env var
    let api_key = settings::get_groq_api_key(app)
        .ok()
        .flatten()
        .or_else(|| std::env::var("GROQ_API_KEY").ok())
        .ok_or_else(|| "Missing Groq API key. Set it in the app settings.".to_string())?;

    emit_log(app, "info", "Transcribing with Groq...");
    let text = transcribe::transcribe_groq(wav_path, api_key.clone())
        .await
        .map_err(|e| e.to_string())?;
    emit_log(app, "info", "Transcription completed");

    // Check if refinement is enabled
    let refine_enabled = settings::get_refine_output_enabled(app)
        .ok()
        .flatten()
        .unwrap_or(false);

    if refine_enabled {
        emit_log(app, "info", "Refining transcription with Qwen model...");
        let custom_prompt = settings::get_refinement_prompt(app)
            .ok()
            .flatten()
            .unwrap_or_default();
        let model = settings::get_refinement_model(app)
            .ok()
            .flatten()
            .unwrap_or_else(|| "qwen/qwen3-32b".to_string());

        match transcribe::refine_transcript(api_key, text.clone(), custom_prompt, model).await {
            Ok(refined_text) => {
                emit_log(app, "info", "Refinement completed successfully");
                Ok(refined_text)
            }
            Err(e) => {
                emit_log(app, "error", format!("Refinement failed: {}. Using original transcript.", e));
                Ok(text)
            }
        }
    } else {
        Ok(text)
    }
}

#[tauri::command]
fn start_recording(state: tauri::State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    do_start_recording(&app, state.inner())
}

#[tauri::command]
async fn stop_and_transcribe(
    state: tauri::State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    let text = do_stop_and_transcribe(&app, state.inner()).await?;
    let _ = app.emit("transcription", text.clone());

    if resolve_auto_type_enabled(&app) {
        match type_text_into_focused_app(&app, &text) {
            Ok(()) => emit_log(&app, "info", "Auto-typed transcription into focused app"),
            Err(e) => emit_log(&app, "error", format!("Auto-type failed: {e}")),
        }
    }

    Ok(text)
}

#[tauri::command]
fn recording_status(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let guard = state.inner().session.lock().map_err(|e| e.to_string())?;
    Ok(guard.is_some())
}

#[tauri::command]
fn set_groq_api_key(app: AppHandle, api_key: String) -> Result<(), String> {
    settings::set_groq_api_key(&app, api_key)
}

#[tauri::command]
fn get_groq_api_key(app: AppHandle) -> Result<Option<String>, String> {
    settings::get_groq_api_key(&app)
}

#[tauri::command]
fn get_hotkey(app: AppHandle) -> Result<String, String> {
    Ok(resolve_hotkey_string(&app))
}

#[tauri::command]
fn set_hotkey(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    hotkey: String,
) -> Result<(), String> {
    let hotkey = hotkey.trim().to_string();
    if hotkey.is_empty() {
        return Err("Hotkey cannot be empty".into());
    }

    // Validate + register first, so we don't persist a broken value.
    register_hotkey(&app, state.inner(), &hotkey)?;
    settings::set_global_hotkey(&app, hotkey)
}

#[tauri::command]
fn reset_hotkey(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<String, String> {
    register_hotkey(&app, state.inner(), DEFAULT_HOTKEY)?;
    settings::set_global_hotkey(&app, DEFAULT_HOTKEY.to_string())?;
    Ok(DEFAULT_HOTKEY.to_string())
}

#[tauri::command]
fn get_trigger_mode(app: AppHandle) -> Result<String, String> {
    Ok(resolve_trigger_mode(&app))
}

#[tauri::command]
fn set_trigger_mode(app: AppHandle, mode: String) -> Result<(), String> {
    settings::set_trigger_mode(&app, mode)
}

#[tauri::command]
fn get_auto_type_enabled(app: AppHandle) -> Result<bool, String> {
    Ok(resolve_auto_type_enabled(&app))
}

#[tauri::command]
fn set_auto_type_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    settings::set_auto_type_enabled(&app, enabled)
}

#[tauri::command]
fn get_type_speed_ms(app: AppHandle) -> Result<u64, String> {
    Ok(resolve_type_speed_ms(&app))
}

#[tauri::command]
fn set_type_speed_ms(app: AppHandle, ms: u64) -> Result<(), String> {
    settings::set_type_speed_ms(&app, ms)
}

#[tauri::command]
fn set_refine_output_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    settings::set_refine_output_enabled(&app, enabled)
}

#[tauri::command]
fn get_refine_output_enabled(app: AppHandle) -> Result<bool, String> {
    Ok(settings::get_refine_output_enabled(&app)?.unwrap_or(false))
}

#[tauri::command]
fn set_refinement_prompt(app: AppHandle, prompt: String) -> Result<(), String> {
    settings::set_refinement_prompt(&app, prompt)
}

#[tauri::command]
fn get_refinement_prompt(app: AppHandle) -> Result<String, String> {
    Ok(settings::get_refinement_prompt(&app)?.unwrap_or_default())
}

#[tauri::command]
fn set_refinement_model(app: AppHandle, model: String) -> Result<(), String> {
    settings::set_refinement_model(&app, model)
}

#[tauri::command]
fn get_refinement_model(app: AppHandle) -> Result<String, String> {
    Ok(settings::get_refinement_model(&app)?.unwrap_or_else(|| "qwen/qwen3-32b".to_string()))
}

#[tauri::command]
fn get_mic_gain(app: AppHandle) -> Result<f32, String> {
    Ok(resolve_mic_gain(&app))
}

#[tauri::command]
fn set_mic_gain(app: AppHandle, gain: f32) -> Result<(), String> {
    settings::set_mic_gain(&app, gain)
}

#[tauri::command]
fn write_clipboard(text: String) -> Result<(), String> {
    set_clipboard_text(&text)
}

#[tauri::command]
fn type_text(app: AppHandle, text: String) -> Result<(), String> {
    type_text_into_focused_app(&app, &text)
}

#[tauri::command]
fn accessibility_status() -> Result<bool, String> {
    Ok(is_accessibility_trusted())
}

#[tauri::command]
fn request_accessibility() -> Result<bool, String> {
    Ok(platform::current().request_accessibility_permission())
}

#[tauri::command]
fn enable_fn_key_listening(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if !is_accessibility_trusted() {
        return Err("Accessibility permission required".into());
    }

    let app_handle = app.clone();
    // Use the platform abstraction's FnKeyListener compatibility shim
    let listener = platform::FnKeyListener::new(move |pressed| {
        let app = app_handle.clone();
        let state_inner = app.state::<AppState>();
        let mode = resolve_trigger_mode(&app);

        if mode == "hold" {
            if pressed {
                // Fn pressed - start recording
                if let Err(e) = do_start_recording(&app, state_inner.inner()) {
                    if e != "Already recording" {
                        emit_log(&app, "error", format!("Failed to start: {e}"));
                    }
                }
            } else {
                // Fn released - stop and transcribe
                let app_clone = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_clone.state::<AppState>();
                    match do_stop_and_transcribe(&app_clone, state.inner()).await {
                        Ok(text) => {
                            let _ = app_clone.emit("transcription", text.clone());
                            if resolve_auto_type_enabled(&app_clone) {
                                if let Err(e) = type_text_into_focused_app(&app_clone, &text) {
                                    emit_log(&app_clone, "error", format!("Auto-type failed: {e}"));
                                }
                            }
                        }
                        Err(e) => emit_log(&app_clone, "error", format!("Stop failed: {e}")),
                    }
                });
            }
        } else if mode == "toggle" {
            if !pressed {
                // Toggle on Fn release
                let app_clone = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_clone.state::<AppState>();
                    let is_recording = state
                        .inner()
                        .session
                        .lock()
                        .ok()
                        .map(|g| g.is_some())
                        .unwrap_or(false);

                    if !is_recording {
                        if let Err(e) = do_start_recording(&app_clone, state.inner()) {
                            emit_log(&app_clone, "error", format!("Failed to start: {e}"));
                        }
                    } else {
                        match do_stop_and_transcribe(&app_clone, state.inner()).await {
                            Ok(text) => {
                                let _ = app_clone.emit("transcription", text.clone());
                                if resolve_auto_type_enabled(&app_clone) {
                                    if let Err(e) = type_text_into_focused_app(&app_clone, &text) {
                                        emit_log(&app_clone, "error", format!("Auto-type failed: {e}"));
                                    }
                                }
                            }
                            Err(e) => emit_log(&app_clone, "error", format!("Stop failed: {e}")),
                        }
                    }
                });
            }
        }
    })?;

    // Store as the old type for compatibility (we wrap it)
    let mut guard = state.inner().fn_listener.lock().map_err(|e| e.to_string())?;
    *guard = Some(FnKeyListenerWrapper(listener));

    emit_log(&app, "info", "Fn key listening enabled");
    Ok(())
}

#[tauri::command]
fn disable_fn_key_listening(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut guard = state.inner().fn_listener.lock().map_err(|e| e.to_string())?;
    if let Some(listener) = guard.take() {
        listener.stop();
        emit_log(&app, "info", "Fn key listening disabled");
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let state = app_handle.state::<AppState>();

            let hotkey_str = resolve_hotkey_string(&app_handle);
            register_hotkey(&app_handle, state.inner(), &hotkey_str)?;

            // ---- Tray menu with Tauri v2 API ----
            let show_item = MenuItemBuilder::with_id("show", "Show GroqBara").build(app)?;
            let hide_item = MenuItemBuilder::with_id("hide", "Hide").build(app)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let start_item = MenuItemBuilder::with_id("start", "Start recording").build(app)?;
            let stop_item = MenuItemBuilder::with_id("stop", "Stop + transcribe").build(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;

            let auto_type_checked = resolve_auto_type_enabled(&app_handle);
            let auto_type_item = CheckMenuItemBuilder::with_id("auto_type", "Auto-type into focused app")
                .checked(auto_type_checked)
                .build(app)?;

            let mode = resolve_trigger_mode(&app_handle);
            let hold_item = CheckMenuItemBuilder::with_id("mode_hold", "Trigger mode: Hold-to-record")
                .checked(mode == "hold")
                .build(app)?;
            let toggle_item = CheckMenuItemBuilder::with_id("mode_toggle", "Trigger mode: Toggle")
                .checked(mode == "toggle")
                .build(app)?;

            let sep3 = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&hide_item)
                .item(&sep1)
                .item(&start_item)
                .item(&stop_item)
                .item(&sep2)
                .item(&auto_type_item)
                .item(&hold_item)
                .item(&toggle_item)
                .item(&sep3)
                .item(&quit_item)
                .build()?;

            let app_handle2 = app_handle.clone();
            
            let tray_icon = app.default_window_icon().cloned()
                .expect("default window icon must be set in tauri.conf.json");

            let tray = TrayIconBuilder::new()
                .icon(tray_icon)
                .icon_as_template(true)  // macOS: auto-adapt to light/dark mode
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    match event.id.as_ref() {
                        "quit" => app.exit(0),
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "hide" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.hide();
                            }
                        }
                        "start" => {
                            let state = app.state::<AppState>();
                            if let Err(e) = do_start_recording(&app_handle2, state.inner()) {
                                emit_log(&app_handle2, "error", format!("Failed to start: {e}"));
                            }
                        }
                        "stop" => {
                            let app_h = app_handle2.clone();
                            tauri::async_runtime::spawn(async move {
                                let state = app_h.state::<AppState>();
                                match do_stop_and_transcribe(&app_h, state.inner()).await {
                                    Ok(text) => {
                                        let _ = app_h.emit("transcription", text.clone());
                                        if resolve_auto_type_enabled(&app_h) {
                                            let _ = type_text_into_focused_app(&app_h, &text);
                                        }
                                    }
                                    Err(e) => emit_log(&app_h, "error", format!("Stop failed: {e}")),
                                }
                            });
                        }
                        "auto_type" => {
                            // Note: CheckMenuItem state is toggled automatically by the OS.
                            // We just read the new state and persist it.
                            // (Dynamic menu API in Tauri v2 is limited; for now we rely on app.state).
                        }
                        "mode_hold" => {
                            let _ = settings::set_trigger_mode(&app_handle2, "hold".into());
                        }
                        "mode_toggle" => {
                            let _ = settings::set_trigger_mode(&app_handle2, "toggle".into());
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            match w.is_visible() {
                                Ok(true) => { let _ = w.hide(); }
                                _ => { let _ = w.show(); let _ = w.set_focus(); }
                            }
                        }
                    }
                })
                .build(app)?;

            // Keep tray alive for app lifetime
            app.manage(tray);

            // ---- Floating recording overlay window ----
            let mut overlay_builder = WebviewWindowBuilder::new(
                app,
                "overlay",
                WebviewUrl::App("src/overlay.html".into()),
            )
            .title("Recording")
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible(false)
            .focused(false);

            // Windows: transparent + no shadow → pill floats with no border.
            // macOS: transparent requires private API, so use a solid dark window instead.
            #[cfg(target_os = "windows")]
            {
                overlay_builder = overlay_builder
                    .inner_size(300.0, 60.0)
                    .transparent(true)
                    .shadow(false);
            }
            #[cfg(target_os = "macos")]
            {
                overlay_builder = overlay_builder
                    .inner_size(200.0, 40.0);
            }

            match overlay_builder.build() {
                Ok(w) => {
                    #[cfg(target_os = "windows")]
                    let (win_w, win_h) = (300.0, 60.0);
                    #[cfg(target_os = "macos")]
                    let (win_w, win_h) = (200.0, 40.0);

                    if let Ok(Some(monitor)) = w.primary_monitor() {
                        let screen = monitor.size();
                        let scale = monitor.scale_factor();
                        let logical_w = screen.width as f64 / scale;
                        let logical_h = screen.height as f64 / scale;
                        let x = (logical_w - win_w) / 2.0;
                        let y = logical_h - win_h - 48.0;
                        let _ = w.set_position(tauri::Position::Logical(
                            tauri::LogicalPosition::new(x, y),
                        ));
                    }
                }
                Err(e) => eprintln!("Failed to create overlay window: {e}"),
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_and_transcribe,
            recording_status,
            set_groq_api_key,
            get_groq_api_key,
            get_hotkey,
            set_hotkey,
            reset_hotkey,
            get_trigger_mode,
            set_trigger_mode,
            get_auto_type_enabled,
            set_auto_type_enabled,
            get_type_speed_ms,
            set_type_speed_ms,
            set_refine_output_enabled,
            get_refine_output_enabled,
            set_refinement_prompt,
            get_refinement_prompt,
            set_refinement_model,
            get_refinement_model,
            get_mic_gain,
            set_mic_gain,
            write_clipboard,
            type_text,
            accessibility_status,
            request_accessibility,
            enable_fn_key_listening,
            disable_fn_key_listening
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
