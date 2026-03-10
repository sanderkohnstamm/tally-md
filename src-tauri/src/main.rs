#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod finished;

use std::path::PathBuf;
use serde::Serialize;
use chrono::Local;

fn todos_dir() -> PathBuf {
    let home = dirs::home_dir().expect("Could not find home directory");
    home.join(".todos")
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
    let _ = std::fs::create_dir_all(&dir);

    let todo = std::fs::read_to_string(dir.join("todo.md")).unwrap_or_default();
    let today_content = std::fs::read_to_string(dir.join("today.md")).unwrap_or_default();
    let done_raw = std::fs::read_to_string(dir.join("done.md")).unwrap_or_default();

    let today_date = Local::now().date_naive();
    let done = if !done_raw.trim().is_empty() {
        finished::fill_empty_days(&done_raw, today_date)
    } else {
        done_raw
    };

    let todo_path = dir.join("todo.md").to_string_lossy().to_string();
    let today_path = dir.join("today.md").to_string_lossy().to_string();
    let done_path = dir.join("done.md").to_string_lossy().to_string();

    FilesPayload { todo, today: today_content, done, todo_path, today_path, done_path }
}

#[tauri::command]
fn save_files(todo: String, today: String, done: String) -> String {
    let dir = todos_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("todo.md"), &todo);
    let _ = std::fs::write(dir.join("today.md"), &today);
    let _ = std::fs::write(dir.join("done.md"), &done);
    "Saved".to_string()
}

#[tauri::command]
fn complete_item(source: String, target: String, cursor_line: usize, to_done: bool) -> CompleteResult {
    if to_done {
        // Moving from today -> done (with breadcrumb + date header)
        let today = Local::now().date_naive();
        match finished::complete_item(&source, &target, cursor_line, today) {
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
        // Moving from todo -> today (simple move)
        match finished::move_item(&source, &target, cursor_line) {
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
        // Recovering from done -> todo (strip breadcrumb)
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
        // Moving from today -> todo (simple move back)
        match finished::move_item(&source, &target, cursor_line) {
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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            load_files,
            save_files,
            complete_item,
            recover_item,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
