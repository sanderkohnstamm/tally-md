/// Logic for completing todos and managing the finished.md file.
use chrono::NaiveDate;
use crate::editor::Buffer;
use crate::markdown;

/// Complete a todo item: remove from todo buffer, add to finished buffer under today's date.
pub fn complete_item(todo_buf: &mut Buffer, finished_buf: &mut Buffer, today: NaiveDate) -> bool {
    let line_idx = todo_buf.cursor.line;
    let line = &todo_buf.lines[line_idx];
    let trimmed = line.trim_start();
    if !trimmed.starts_with("- ") {
        return false;
    }

    let text = trimmed.strip_prefix("- ").unwrap();
    let breadcrumb = markdown::breadcrumb_for(&todo_buf.lines, line_idx);

    let entry = if breadcrumb.is_empty() {
        format!("- {}", text)
    } else {
        format!("- {} ({})", text, breadcrumb.join(" > "))
    };

    // Remove line from todo
    todo_buf.force_undo_snapshot();
    if todo_buf.lines.len() > 1 {
        todo_buf.lines.remove(line_idx);
        if todo_buf.cursor.line >= todo_buf.lines.len() {
            todo_buf.cursor.line = todo_buf.lines.len() - 1;
        }
        todo_buf.cursor.col = todo_buf.cursor.col.min(
            todo_buf.lines[todo_buf.cursor.line].chars().count()
        );
    } else {
        todo_buf.lines[0] = String::new();
        todo_buf.cursor.col = 0;
    }
    todo_buf.dirty = true;

    // Add to finished buffer under today's header
    let date_header = format!("## {}", today.format("%Y-%m-%d"));
    insert_into_finished(finished_buf, &date_header, &entry);
    true
}

/// Recover a finished item: remove from finished buffer, append to todo buffer.
pub fn recover_item(finished_buf: &mut Buffer, todo_buf: &mut Buffer) -> bool {
    let line_idx = finished_buf.cursor.line;
    let line = finished_buf.lines[line_idx].clone();
    let trimmed = line.trim_start();
    if !trimmed.starts_with("- ") {
        return false;
    }

    // Parse the text (strip breadcrumb if present)
    let text = trimmed.strip_prefix("- ").unwrap();
    let clean_text = if let Some(paren_start) = text.rfind(" (") {
        if text.ends_with(')') {
            text[..paren_start].to_string()
        } else {
            text.to_string()
        }
    } else {
        text.to_string()
    };

    // Remove from finished
    finished_buf.force_undo_snapshot();
    if finished_buf.lines.len() > 1 {
        finished_buf.lines.remove(line_idx);
        if finished_buf.cursor.line >= finished_buf.lines.len() {
            finished_buf.cursor.line = finished_buf.lines.len() - 1;
        }
    } else {
        finished_buf.lines[0] = String::new();
        finished_buf.cursor.col = 0;
    }
    finished_buf.dirty = true;

    // Append to end of todo buffer
    let new_line = format!("- {}", clean_text);
    let last = todo_buf.lines.len();
    if last == 1 && todo_buf.lines[0].is_empty() {
        todo_buf.lines[0] = new_line;
    } else {
        todo_buf.lines.push(new_line);
    }
    todo_buf.dirty = true;
    true
}

fn insert_into_finished(buf: &mut Buffer, date_header: &str, entry: &str) {
    buf.force_undo_snapshot();

    // Find the date header
    let header_idx = buf.lines.iter().position(|l| l == date_header);

    if let Some(idx) = header_idx {
        // Insert after the header (and any existing items under it)
        let mut insert_at = idx + 1;
        while insert_at < buf.lines.len() {
            let line = &buf.lines[insert_at];
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("## ") {
                break;
            }
            insert_at += 1;
        }
        buf.lines.insert(insert_at, entry.to_string());
    } else {
        // Create new date header at the top
        let mut insert_at = 0;
        // Skip any leading blank lines
        while insert_at < buf.lines.len() && buf.lines[insert_at].trim().is_empty() {
            insert_at += 1;
        }
        if insert_at > 0 || (!buf.lines.is_empty() && !buf.lines[0].is_empty()) {
            buf.lines.insert(insert_at, String::new());
            buf.lines.insert(insert_at, entry.to_string());
            buf.lines.insert(insert_at, date_header.to_string());
        } else {
            // Empty file
            buf.lines = vec![
                date_header.to_string(),
                entry.to_string(),
            ];
        }
    }
    buf.dirty = true;
}

/// Fill in empty day headers between the oldest and today.
pub fn fill_empty_days(buf: &mut Buffer, today: NaiveDate) {
    // Parse existing dates
    let mut dates: Vec<NaiveDate> = Vec::new();
    for line in &buf.lines {
        if let Some(date_str) = line.strip_prefix("## ") {
            if let Ok(date) = NaiveDate::parse_from_str(date_str.trim(), "%Y-%m-%d") {
                dates.push(date);
            }
        }
    }

    if dates.is_empty() {
        return;
    }

    dates.sort();
    let oldest = *dates.first().unwrap();
    let newest = if today > *dates.last().unwrap() { today } else { *dates.last().unwrap() };

    let mut date = oldest;
    while date <= newest {
        if !dates.contains(&date) {
            // Find where to insert (maintain newest-first order at top, or oldest-first)
            // Actually, just append at the right position to maintain chronological order
            let header = format!("## {}", date.format("%Y-%m-%d"));
            // Find the right position: after the previous date's section
            let insert_pos = find_date_insert_position(&buf.lines, date);
            buf.lines.insert(insert_pos, String::new());
            buf.lines.insert(insert_pos, header);
        }
        date += chrono::Duration::days(1);
    }
}

fn find_date_insert_position(lines: &[String], date: NaiveDate) -> usize {
    // Find position to insert so dates are newest-first
    for (i, line) in lines.iter().enumerate() {
        if let Some(date_str) = line.strip_prefix("## ") {
            if let Ok(existing) = NaiveDate::parse_from_str(date_str.trim(), "%Y-%m-%d") {
                if date > existing {
                    return i;
                }
            }
        }
    }
    lines.len()
}

/// Migrate: if todo.md has ## YYYY-MM-DD sections, split them out to finished.md
pub fn migrate_inline_log(todo_content: &str) -> (String, String) {
    let mut todo_lines = Vec::new();
    let mut finished_lines = Vec::new();
    let mut in_log = false;

    for line in todo_content.lines() {
        if !in_log {
            if let Some(date_str) = line.strip_prefix("## ") {
                if NaiveDate::parse_from_str(date_str.trim(), "%Y-%m-%d").is_ok() {
                    in_log = true;
                    finished_lines.push(line.to_string());
                    continue;
                }
            }
            todo_lines.push(line.to_string());
        } else {
            finished_lines.push(line.to_string());
        }
    }

    // Trim trailing blank lines from todo
    while todo_lines.last().map_or(false, |l| l.trim().is_empty()) {
        todo_lines.pop();
    }

    (todo_lines.join("\n"), finished_lines.join("\n"))
}
