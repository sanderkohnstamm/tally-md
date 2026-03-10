use chrono::NaiveDate;

/// Simple move: take a list item from source and append to target.
/// Returns (new_source, new_target).
pub fn move_item(
    source_text: &str,
    target_text: &str,
    cursor_line: usize,
) -> Option<(String, String)> {
    let source_lines: Vec<&str> = source_text.lines().collect();
    if cursor_line >= source_lines.len() {
        return None;
    }

    let line = source_lines[cursor_line];
    let trimmed = line.trim_start();
    if !trimmed.starts_with("- ") {
        return None;
    }

    // Remove line from source
    let mut new_source: Vec<&str> = source_lines.clone();
    new_source.remove(cursor_line);
    if new_source.is_empty() {
        new_source.push("");
    }

    // Append to target
    let mut target = target_text.to_string();
    if target.trim().is_empty() {
        target = trimmed.to_string();
    } else {
        target.push('\n');
        target.push_str(trimmed);
    }

    Some((new_source.join("\n"), target))
}

/// Complete a todo item: given the full todo text and cursor line,
/// returns (new_todo_text, new_finished_text).
pub fn complete_item(
    todo_text: &str,
    finished_text: &str,
    cursor_line: usize,
    today: NaiveDate,
) -> Option<(String, String)> {
    let todo_lines: Vec<&str> = todo_text.lines().collect();
    if cursor_line >= todo_lines.len() {
        return None;
    }

    let line = todo_lines[cursor_line];
    let trimmed = line.trim_start();
    if !trimmed.starts_with("- ") {
        return None;
    }

    let text = trimmed.strip_prefix("- ").unwrap();
    let breadcrumb = breadcrumb_for(&todo_lines, cursor_line);

    let entry = if breadcrumb.is_empty() {
        format!("- {}", text)
    } else {
        format!("- {} ({})", text, breadcrumb.join(" > "))
    };

    // Remove line from todo
    let mut new_todo: Vec<&str> = todo_lines.clone();
    new_todo.remove(cursor_line);
    if new_todo.is_empty() {
        new_todo.push("");
    }

    // Add to finished under today's header
    let new_finished = insert_into_finished(finished_text, today, &entry);

    Some((new_todo.join("\n"), new_finished))
}

/// Recover a finished item back to todo.
pub fn recover_item(
    finished_text: &str,
    todo_text: &str,
    cursor_line: usize,
) -> Option<(String, String)> {
    let finished_lines: Vec<&str> = finished_text.lines().collect();
    if cursor_line >= finished_lines.len() {
        return None;
    }

    let line = finished_lines[cursor_line];
    let trimmed = line.trim_start();
    if !trimmed.starts_with("- ") {
        return None;
    }

    // Strip breadcrumb if present
    let text = trimmed.strip_prefix("- ").unwrap();
    let clean_text = if let Some(paren_start) = text.rfind(" (") {
        if text.ends_with(')') {
            &text[..paren_start]
        } else {
            text
        }
    } else {
        text
    };

    // Remove from finished
    let mut new_finished: Vec<&str> = finished_lines.clone();
    new_finished.remove(cursor_line);
    if new_finished.is_empty() {
        new_finished.push("");
    }

    // Append to todo
    let new_line = format!("- {}", clean_text);
    let mut todo = todo_text.to_string();
    if todo.trim().is_empty() {
        todo = new_line;
    } else {
        todo.push('\n');
        todo.push_str(&new_line);
    }

    Some((new_finished.join("\n"), todo))
}

/// Fill empty day headers between oldest existing date and today.
pub fn fill_empty_days(text: &str, today: NaiveDate) -> String {
    let lines: Vec<&str> = text.lines().collect();

    let mut dates: Vec<NaiveDate> = Vec::new();
    for line in &lines {
        if let Some(date_str) = line.strip_prefix("## ") {
            if let Ok(date) = NaiveDate::parse_from_str(date_str.trim(), "%Y-%m-%d") {
                dates.push(date);
            }
        }
    }

    if dates.is_empty() {
        return text.to_string();
    }

    dates.sort();
    let oldest = *dates.first().unwrap();
    let newest = if today > *dates.last().unwrap() {
        today
    } else {
        *dates.last().unwrap()
    };

    let mut result_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

    let mut date = oldest;
    while date <= newest {
        if !dates.contains(&date) {
            let header = format!("## {}", date.format("%Y-%m-%d"));
            let pos = find_date_insert_position(&result_lines, date);
            result_lines.insert(pos, String::new());
            result_lines.insert(pos, header);
        }
        date += chrono::Duration::days(1);
    }

    result_lines.join("\n")
}

fn insert_into_finished(finished_text: &str, today: NaiveDate, entry: &str) -> String {
    let mut lines: Vec<String> = if finished_text.trim().is_empty() {
        Vec::new()
    } else {
        finished_text.lines().map(|l| l.to_string()).collect()
    };

    let date_header = format!("## {}", today.format("%Y-%m-%d"));
    let header_idx = lines.iter().position(|l| l == &date_header);

    if let Some(idx) = header_idx {
        let mut insert_at = idx + 1;
        while insert_at < lines.len() {
            let trimmed = lines[insert_at].trim();
            if trimmed.is_empty() || trimmed.starts_with("## ") {
                break;
            }
            insert_at += 1;
        }
        lines.insert(insert_at, entry.to_string());
    } else if lines.is_empty() {
        lines.push(date_header);
        lines.push(entry.to_string());
    } else {
        // Insert at top (newest first)
        lines.insert(0, String::new());
        lines.insert(0, entry.to_string());
        lines.insert(0, date_header);
    }

    lines.join("\n")
}

fn find_date_insert_position(lines: &[String], date: NaiveDate) -> usize {
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

fn breadcrumb_for(lines: &[&str], line_idx: usize) -> Vec<String> {
    let line = lines[line_idx];
    let indent = line.len() - line.trim_start().len();
    if indent == 0 {
        // Look for heading above
        for i in (0..line_idx).rev() {
            let l = lines[i].trim_start();
            if l.starts_with("# ") || l.starts_with("## ") || l.starts_with("### ") {
                let text = l.trim_start_matches('#').trim();
                return vec![text.to_string()];
            }
        }
        return Vec::new();
    }

    let mut crumbs = Vec::new();
    let mut current_indent = indent;

    for i in (0..line_idx).rev() {
        let l = lines[i];
        let l_indent = l.len() - l.trim_start().len();
        let trimmed = l.trim_start();

        if l_indent < current_indent && trimmed.starts_with("- ") {
            let text = trimmed.strip_prefix("- ").unwrap();
            crumbs.push(text.to_string());
            current_indent = l_indent;
            if current_indent == 0 {
                break;
            }
        }

        if trimmed.starts_with('#') && current_indent > 0 {
            let text = trimmed.trim_start_matches('#').trim();
            crumbs.push(text.to_string());
            break;
        }
    }

    crumbs.reverse();
    crumbs
}
