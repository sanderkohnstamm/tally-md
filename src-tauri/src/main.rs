#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod finished;
mod git_sync;
mod settings;

use std::path::PathBuf;
use serde::Serialize;
use chrono::Local;

fn todos_dir() -> PathBuf {
    let s = settings::load();
    let path = if s.storage_mode == "git" && !s.git_repo_name.is_empty() {
        // Git repos are cloned into ~/.tallymd/repos/<repo_name>/
        let base = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".tallymd")
            .join("repos")
            .join(&s.git_repo_name);
        base
    } else {
        PathBuf::from(&s.local_path)
    };
    let _ = std::fs::create_dir_all(&path);
    path
}

#[derive(Serialize)]
struct FilesPayload {
    todo: String,
    today: String,
    done: String,
    todo_path: String,
    today_path: String,
    done_path: String,
}

#[derive(Serialize)]
struct CompleteResult {
    source: String,
    target: String,
    message: String,
}

#[tauri::command]
fn load_files() -> FilesPayload {
    let dir = todos_dir();
    let settings = settings::load();

    let todo = std::fs::read_to_string(dir.join("todo.md")).unwrap_or_default();
    let today_content = std::fs::read_to_string(dir.join("today.md")).unwrap_or_default();
    let done_raw = std::fs::read_to_string(dir.join("done.md")).unwrap_or_default();

    let today_date = Local::now().date_naive();
    let done = if !done_raw.trim().is_empty() {
        finished::fill_empty_days(&done_raw, today_date, &settings.date_format)
    } else {
        done_raw
    };

    let todo_path = dir.join("todo.md").to_string_lossy().to_string();
    let today_path = dir.join("today.md").to_string_lossy().to_string();
    let done_path = dir.join("done.md").to_string_lossy().to_string();

    FilesPayload { todo, today: today_content, done, todo_path, today_path, done_path }
}

#[tauri::command]
fn save_files(todo: String, today: String, done: String) -> Result<String, String> {
    let dir = todos_dir();
    std::fs::write(dir.join("todo.md"), &todo)
        .map_err(|e| format!("Failed to save todo.md: {}", e))?;
    std::fs::write(dir.join("today.md"), &today)
        .map_err(|e| format!("Failed to save today.md: {}", e))?;
    std::fs::write(dir.join("done.md"), &done)
        .map_err(|e| format!("Failed to save done.md: {}", e))?;
    Ok("Saved".to_string())
}

#[tauri::command]
fn complete_item(source: String, target: String, cursor_line: usize, to_done: bool) -> CompleteResult {
    let settings = settings::load();
    if to_done {
        let today = Local::now().date_naive();
        match finished::complete_item(&source, &target, cursor_line, today, &settings.date_format) {
            Some((new_source, new_target)) => CompleteResult {
                source: new_source,
                target: new_target,
                message: "Done!".to_string(),
            },
            None => CompleteResult {
                source,
                target,
                message: "Not a list item (- )".to_string(),
            },
        }
    } else {
        match finished::move_item_forward(&source, &target, cursor_line) {
            Some((new_source, new_target)) => CompleteResult {
                source: new_source,
                target: new_target,
                message: "Moved to today".to_string(),
            },
            None => CompleteResult {
                source,
                target,
                message: "Not a list item (- )".to_string(),
            },
        }
    }
}

#[tauri::command]
fn recover_item(source: String, target: String, cursor_line: usize, from_done: bool) -> CompleteResult {
    if from_done {
        match finished::recover_item(&source, &target, cursor_line) {
            Some((new_source, new_target)) => CompleteResult {
                source: new_source,
                target: new_target,
                message: "Recovered to todo".to_string(),
            },
            None => CompleteResult {
                source,
                target,
                message: "Not a list item".to_string(),
            },
        }
    } else {
        match finished::move_item_back(&source, &target, cursor_line) {
            Some((new_source, new_target)) => CompleteResult {
                source: new_source,
                target: new_target,
                message: "Moved back to todo".to_string(),
            },
            None => CompleteResult {
                source,
                target,
                message: "Not a list item".to_string(),
            },
        }
    }
}

#[tauri::command]
fn reformat_dates(done_content: String, new_format: String) -> String {
    finished::reformat_date_headers(&done_content, &new_format)
}

#[tauri::command]
fn load_settings() -> settings::Settings {
    settings::load()
}

#[tauri::command]
fn save_settings(
    storage_mode: String,
    local_path: String,
    git_repo: String,
    git_repo_name: String,
    theme_index: usize,
    date_format: String,
    layout: String,
    pane_sizes: Vec<f64>,
    sync_interval: u64,
    setup_done: bool,
    keybindings: std::collections::HashMap<String, String>,
) -> Result<String, String> {
    let s = settings::Settings {
        storage_mode,
        local_path,
        git_repo,
        git_repo_name,
        theme_index,
        date_format,
        layout,
        pane_sizes,
        sync_interval,
        setup_done,
        keybindings,
    };
    settings::save(&s)?;
    Ok("Settings saved".to_string())
}

// --- Git sync commands ---

#[tauri::command]
fn git_init_repo(repo_url: String, repo_name: String) -> Result<String, String> {
    let token = git_sync::get_token()?;
    let base = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".tallymd")
        .join("repos")
        .join(&repo_name);
    let local_path = base.to_string_lossy().to_string();
    git_sync::init_repo(&repo_url, &local_path, &token)
}

#[tauri::command]
fn git_store_token(token: String) -> Result<String, String> {
    git_sync::store_token(&token)?;
    Ok("Token stored".to_string())
}

#[tauri::command]
fn git_has_token() -> bool {
    git_sync::has_token()
}

#[tauri::command]
fn git_delete_token() -> Result<String, String> {
    git_sync::delete_token()?;
    Ok("Token deleted".to_string())
}

#[tauri::command]
fn git_pull() -> Result<String, String> {
    let s = settings::load();
    if s.storage_mode != "git" || s.git_repo.is_empty() {
        return Err("Git sync not configured".to_string());
    }
    let token = git_sync::get_token()?;
    let local_path = todos_dir().to_string_lossy().to_string();
    git_sync::pull(&s.git_repo, &local_path, &token)
}

#[tauri::command]
fn git_push() -> Result<String, String> {
    let s = settings::load();
    if s.storage_mode != "git" || s.git_repo.is_empty() {
        return Err("Git sync not configured".to_string());
    }
    let token = git_sync::get_token()?;
    let local_path = todos_dir().to_string_lossy().to_string();
    git_sync::commit_and_push(&s.git_repo, &local_path, &token)
}

#[tauri::command]
fn git_sync_full() -> Result<String, String> {
    let s = settings::load();
    if s.storage_mode != "git" || s.git_repo.is_empty() {
        return Err("Git sync not configured".to_string());
    }
    let token = git_sync::get_token()?;
    let local_path = todos_dir().to_string_lossy().to_string();

    // Pull first, then commit+push
    let pull_msg = git_sync::pull(&s.git_repo, &local_path, &token)?;
    let push_msg = git_sync::commit_and_push(&s.git_repo, &local_path, &token)?;

    Ok(format!("{} | {}", pull_msg, push_msg))
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(&url).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&url).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd").args(["/c", "start", &url]).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            load_files,
            save_files,
            complete_item,
            recover_item,
            reformat_dates,
            load_settings,
            save_settings,
            git_init_repo,
            git_store_token,
            git_has_token,
            git_delete_token,
            git_pull,
            git_push,
            git_sync_full,
            open_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
