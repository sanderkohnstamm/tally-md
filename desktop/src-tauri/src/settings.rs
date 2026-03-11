use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub storage_mode: String,  // "local" or "git"
    pub local_path: String,    // custom path for local storage
    pub git_repo: String,      // git repo URL
    pub git_repo_name: String, // repo name for git storage (e.g. "tally-md-log", "tally-md-work")
    pub theme_index: usize,    // index into palettes array
    pub date_format: String,   // e.g. "%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y"
    pub layout: String,        // "horizontal" or "vertical"
    pub pane_sizes: Vec<f64>,  // [todo%, today%, done%] — stored as 0-100
    pub sync_interval: u64,    // auto-sync interval in minutes (0 = disabled)
    pub setup_done: bool,      // whether first-time setup has been completed
    #[serde(default = "default_keybindings")]
    pub keybindings: HashMap<String, String>, // action -> key string
}

pub fn default_keybindings() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".to_string(), "Mod-s".to_string());
    m.insert("moveForward".to_string(), "Mod-Enter".to_string());
    m.insert("sendBack".to_string(), "Mod-Shift-Enter".to_string());
    m.insert("skipToDone".to_string(), "Mod-Shift-d".to_string());
    m.insert("bold".to_string(), "Mod-b".to_string());
    m.insert("italic".to_string(), "Mod-i".to_string());
    m.insert("toggleFold".to_string(), "Mod-e".to_string());
    m.insert("toggleFoldAll".to_string(), "Mod-Shift-e".to_string());
    m.insert("cyclePane".to_string(), "Mod-\\".to_string());
    m.insert("cyclePaneBack".to_string(), "Mod-Shift-\\".to_string());
    m.insert("toggleDonePane".to_string(), "Mod-Shift-b".to_string());
    m.insert("sync".to_string(), "Mod-Shift-s".to_string());
    m.insert("cycleTheme".to_string(), "Mod-k".to_string());
    m.insert("openSettings".to_string(), "Mod-,".to_string());
    m
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
            git_repo_name: "tally-md-log".to_string(),
            theme_index: 0,
            date_format: "%Y-%m-%d".to_string(),
            layout: "horizontal".to_string(),
            pane_sizes: vec![40.0, 30.0, 30.0],
            sync_interval: 5,
            setup_done: false,
            keybindings: default_keybindings(),
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
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Failed to write settings: {e}"))?;

    // Also save to repo dir so settings sync with git
    save_to_repo(settings)?;

    Ok(())
}

fn save_to_repo(settings: &Settings) -> Result<(), String> {
    let repo_dir = if settings.storage_mode == "git" && !settings.git_repo_name.is_empty() {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".tallymd")
            .join("repos")
            .join(&settings.git_repo_name)
    } else {
        PathBuf::from(&settings.local_path)
    };

    if repo_dir.exists() {
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;
        let _ = std::fs::write(repo_dir.join("settings.json"), json);
    }
    Ok(())
}
