use arboard::Clipboard;
use serde::Serialize;
use std::{str::FromStr, time::Duration};
use tauri::{
    AppHandle, Emitter, Manager, Runtime,
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

mod auto_type;
mod fn_key_listener;
mod recorder;
mod settings;
mod transcribe;

#[derive(Default)]
struct AppState {
    session: std::sync::Mutex<Option<recorder::RecordingSession>>,
    hotkey: std::sync::Mutex<Option<Shortcut>>,
    fn_listener: std::sync::Mutex<Option<fn_key_listener::FnKeyListener>>,
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

// Default shortcut:
// NOTE: "Fn" cannot be registered as a global shortcut by most OS/hotkey backends.
// We default to F13, which is a common "extra" function key on macOS keyboards.
const DEFAULT_HOTKEY: &str = "F13";

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

fn resolve_trigger_mode<R: Runtime>(app: &AppHandle<R>) -> String {
    settings::get_trigger_mode(app)
        .ok()
        .flatten()
        .unwrap_or_else(|| DEFAULT_TRIGGER_MODE.to_string())
}

#[cfg(target_os = "macos")]
fn is_accessibility_trusted() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}

#[cfg(target_os = "macos")]
fn request_accessibility_permission() -> bool {
    use std::ffi::c_void;
    
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

#[cfg(not(target_os = "macos"))]
fn is_accessibility_trusted() -> bool {
    // On non-macOS, don't block MVP UX on this check.
    true
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
    auto_type::type_text(text, delay)
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
    let session = recorder::RecordingSession::start().map_err(|e| e.to_string())?;
    *guard = Some(session);
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

    emit_log(app, "info", "Stopping recording...");
    let wav_path = session.stop_and_save_wav().map_err(|e| e.to_string())?;
    emit_log(app, "info", format!("Saved WAV: {}", wav_path.display()));

    // API key resolution: settings.json > env var
    let api_key = settings::get_groq_api_key(app)
        .ok()
        .flatten()
        .or_else(|| std::env::var("GROQ_API_KEY").ok())
        .ok_or_else(|| "Missing Groq API key. Set it in the app settings.".to_string())?;

    emit_log(app, "info", "Transcribing with Groq...");
    let text = transcribe::transcribe_groq(wav_path, api_key)
        .await
        .map_err(|e| e.to_string())?;
    emit_log(app, "info", "Transcription completed");

    Ok(text)
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
    #[cfg(target_os = "macos")]
    {
        Ok(request_accessibility_permission())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
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
    let listener = fn_key_listener::FnKeyListener::new(move |pressed| {
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

    let mut guard = state.inner().fn_listener.lock().map_err(|e| e.to_string())?;
    *guard = Some(listener);

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
            let show_item = MenuItemBuilder::with_id("show", "Show GroqTranscriber").build(app)?;
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
            
            let tray = TrayIconBuilder::new()
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
