use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub storage_mode: String,       // "local" or "git"
    pub local_path: String,         // custom path for local storage
    pub git_repo: String,           // git repo URL
    pub theme_index: usize,         // index into palettes array
    pub date_format: String,        // e.g. "%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y"
    pub layout: String,             // "horizontal" or "vertical"
    pub pane_sizes: Vec<f64>,       // [todo%, today%, done%] — stored as 0-100
    pub sync_interval: u64,         // auto-sync interval in minutes (0 = disabled)
    pub setup_done: bool,           // whether first-time setup has been completed
}

impl Default for Settings {
    fn default() -> Self {
        let default_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".todos")
            .to_string_lossy()
            .to_string();

        Settings {
            storage_mode: "local".to_string(),
            local_path: default_path,
            git_repo: String::new(),
            theme_index: 0,
            date_format: "%Y-%m-%d".to_string(),
            layout: "horizontal".to_string(),
            pane_sizes: vec![40.0, 30.0, 30.0],
            sync_interval: 5,
            setup_done: false,
        }
    }
}

fn settings_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
    let app_dir = config_dir.join("tallymd");
    let _ = std::fs::create_dir_all(&app_dir);
    app_dir.join("settings.json")
}

pub fn load() -> Settings {
    let path = settings_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(settings: &Settings) -> Result<(), String> {
    let path = settings_path();
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write settings: {}", e))?;
    Ok(())
}
