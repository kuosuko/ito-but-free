use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager, Runtime};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Settings {
    #[serde(default)]
    pub groq_api_key: Option<String>,

    /// Global hotkey string, e.g. "CommandOrControl+Shift+R".
    #[serde(default)]
    pub global_hotkey: Option<String>,

    /// How the hotkey behaves:
    /// - "hold": press to start recording, release to stop+transcribe
    /// - "toggle": press once to start, press again to stop+transcribe
    #[serde(default)]
    pub trigger_mode: Option<String>,

    /// Whether we should auto-type the transcription into the focused app.
    /// Stored as `auto_type_enabled` in settings.json.
    #[serde(default)]
    pub auto_type_enabled: Option<bool>,

    /// Delay between characters in milliseconds (0 = as fast as possible).
    /// Stored as `type_speed_ms` in settings.json.
    #[serde(default)]
    pub type_speed_ms: Option<u64>,

    // ---- Legacy fields kept for backwards compatibility (do not write new values) ----
    /// Legacy: Automatically insert the transcription.
    #[serde(default, skip_serializing)]
    pub auto_insert: Option<bool>,

    /// Legacy: How to insert the transcription.
    #[serde(default, skip_serializing)]
    pub insert_mode: Option<String>,
}

fn settings_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to resolve app_config_dir: {e}"))?;
    Ok(dir.join("settings.json"))
}

pub fn load<R: Runtime>(app: &AppHandle<R>) -> Result<Settings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    let bytes = fs::read(&path).map_err(|e| format!("Failed to read settings: {e}"))?;
    let mut s: Settings =
        serde_json::from_slice(&bytes).map_err(|e| format!("Failed to parse settings: {e}"))?;

    // Migrate legacy fields if the new ones are absent.
    if s.auto_type_enabled.is_none() {
        s.auto_type_enabled = s.auto_insert;
    }
    if s.type_speed_ms.is_none() {
        // No legacy equivalent.
        s.type_speed_ms = Some(0);
    }

    Ok(s)
}

pub fn set_trigger_mode<R: Runtime>(app: &AppHandle<R>, mode: String) -> Result<(), String> {
    let mut s = load(app)?;
    let m = mode.trim().to_lowercase();
    if m != "hold" && m != "toggle" {
        return Err("trigger_mode must be 'hold' or 'toggle'".into());
    }
    s.trigger_mode = Some(m);
    save(app, &s)
}

pub fn get_trigger_mode<R: Runtime>(app: &AppHandle<R>) -> Result<Option<String>, String> {
    Ok(load(app)?.trigger_mode)
}

pub fn save<R: Runtime>(app: &AppHandle<R>, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create settings dir: {e}"))?;
    }
    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    fs::write(&path, bytes).map_err(|e| format!("Failed to write settings: {e}"))
}

pub fn set_groq_api_key<R: Runtime>(app: &AppHandle<R>, key: String) -> Result<(), String> {
    let mut s = load(app)?;
    let trimmed = key.trim().to_string();
    if trimmed.is_empty() {
        s.groq_api_key = None;
    } else {
        s.groq_api_key = Some(trimmed);
    }
    save(app, &s)
}

pub fn get_groq_api_key<R: Runtime>(app: &AppHandle<R>) -> Result<Option<String>, String> {
    Ok(load(app)?.groq_api_key)
}

pub fn set_global_hotkey<R: Runtime>(app: &AppHandle<R>, hotkey: String) -> Result<(), String> {
    let mut s = load(app)?;
    let trimmed = hotkey.trim().to_string();
    if trimmed.is_empty() {
        s.global_hotkey = None;
    } else {
        s.global_hotkey = Some(trimmed);
    }
    save(app, &s)
}

pub fn get_global_hotkey<R: Runtime>(app: &AppHandle<R>) -> Result<Option<String>, String> {
    Ok(load(app)?.global_hotkey)
}

pub fn set_auto_type_enabled<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> Result<(), String> {
    let mut s = load(app)?;
    s.auto_type_enabled = Some(enabled);
    save(app, &s)
}

pub fn get_auto_type_enabled<R: Runtime>(app: &AppHandle<R>) -> Result<Option<bool>, String> {
    Ok(load(app)?.auto_type_enabled)
}

pub fn set_type_speed_ms<R: Runtime>(app: &AppHandle<R>, ms: u64) -> Result<(), String> {
    let mut s = load(app)?;
    s.type_speed_ms = Some(ms);
    save(app, &s)
}

pub fn get_type_speed_ms<R: Runtime>(app: &AppHandle<R>) -> Result<Option<u64>, String> {
    Ok(load(app)?.type_speed_ms)
}
