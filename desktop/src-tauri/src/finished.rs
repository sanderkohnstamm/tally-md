use chrono::NaiveDate;

/// Move item forward (todo→today): add parent breadcrumb in brackets.
/// Returns `(new_source, new_target)`.
pub fn move_item_forward(
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

    let text = trimmed.strip_prefix("- ").unwrap();
    let breadcrumb = breadcrumb_for(&source_lines, cursor_line);

    let entry = if breadcrumb.is_empty() {
        format!("- {text}")
    } else {
        format!("- {text} ({})", breadcrumb.join(" > "))
    };

    // Remove line from source
    let mut new_source: Vec<&str> = source_lines.clone();
    new_source.remove(cursor_line);
    if new_source.is_empty() {
        new_source.push("");
    }

    // Insert into target at first blank line
    let target = insert_at_first_gap(target_text, &entry);

    Some((new_source.join("\n"), target))
}

/// Move item backward (today→todo): strip breadcrumb and place under matching parent.
/// Returns `(new_source, new_target)`.
pub fn move_item_back(
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

    let text = trimmed.strip_prefix("- ").unwrap();

    // Extract breadcrumb and clean text
    let (clean_text, breadcrumb) = if let Some(paren_start) = text.rfind(" (") {
        if text.ends_with(')') {
            let crumb = &text[paren_start + 2..text.len() - 1];
            (&text[..paren_start], Some(crumb.to_string()))
        } else {
            (text, None)
        }
    } else {
        (text, None)
    };

    // Remove from source
    let mut new_source: Vec<&str> = source_lines.clone();
    new_source.remove(cursor_line);
    if new_source.is_empty() {
        new_source.push("");
    }

    // Insert into target under matching parent, or at first gap
    let new_line = format!("- {clean_text}");

    let target = if let Some(ref crumb) = breadcrumb {
        let mut target_lines: Vec<String> = if target_text.trim().is_empty() {
            Vec::new()
        } else {
            target_text.lines().map(ToString::to_string).collect()
        };
        if let Some(pos) = find_parent_position(&target_lines, crumb) {
            target_lines.insert(pos, new_line);
            ensure_trailing_blank_line(target_lines).join("\n")
        } else {
            insert_at_first_gap(target_text, &new_line)
        }
    } else {
        insert_at_first_gap(target_text, &new_line)
    };

    Some((new_source.join("\n"), target))
}

/// Find position to insert an item below its parent heading/item in the target.
/// Breadcrumb can be "Heading" or "Heading > Parent item".
fn find_parent_position(lines: &[String], breadcrumb: &str) -> Option<usize> {
    let parts: Vec<&str> = breadcrumb.split(" > ").collect();

    // First, try to find the heading
    let heading = parts[0];
    let mut heading_idx = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            let text = trimmed.trim_start_matches('#').trim();
            if text == heading {
                heading_idx = Some(i);
                break;
            }
        }
    }

    let heading_idx = heading_idx?;

    // If breadcrumb has more parts, find the last parent item under that heading
    if parts.len() > 1 {
        let parent_item = *parts.last().unwrap();
        // Search below the heading for the parent item
        for i in (heading_idx + 1)..lines.len() {
            let trimmed = lines[i].trim_start();
            if trimmed.starts_with('#') {
                break; // Hit next heading
            }
            if trimmed.starts_with("- ") {
                let item_text = trimmed.strip_prefix("- ").unwrap();
                if item_text == parent_item {
                    // Insert after this parent's children
                    let parent_indent = lines[i].len() - lines[i].trim_start().len();
                    let mut insert_at = i + 1;
                    while insert_at < lines.len() {
                        let l = &lines[insert_at];
                        let l_indent = l.len() - l.trim_start().len();
                        if l.trim().is_empty() || l_indent <= parent_indent {
                            break;
                        }
                        insert_at += 1;
                    }
                    return Some(insert_at);
                }
            }
        }
    }

    // Just insert at end of the heading's section
    let mut insert_at = heading_idx + 1;
    while insert_at < lines.len() {
        let trimmed = lines[insert_at].trim_start();
        if trimmed.starts_with('#') {
            break;
        }
        insert_at += 1;
    }
    Some(insert_at)
}

/// Complete a todo item: given the full todo text and cursor line,
/// returns `(new_todo_text, new_finished_text)`.
pub fn complete_item(
    todo_text: &str,
    finished_text: &str,
    cursor_line: usize,
    today: NaiveDate,
    date_format: &str,
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
        format!("- {text}")
    } else {
        format!("- {text} ({})", breadcrumb.join(" > "))
    };

    // Remove line from todo
    let mut new_todo: Vec<&str> = todo_lines.clone();
    new_todo.remove(cursor_line);
    if new_todo.is_empty() {
        new_todo.push("");
    }

    // Add to finished under today's header
    let new_finished = insert_into_finished(finished_text, today, &entry, date_format);

    Some((new_todo.join("\n"), new_finished))
}

/// Recover a finished item back to todo: strip breadcrumb and place under matching parent.
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

    let text = trimmed.strip_prefix("- ").unwrap();

    // Extract breadcrumb and clean text
    let (clean_text, breadcrumb) = if let Some(paren_start) = text.rfind(" (") {
        if text.ends_with(')') {
            let crumb = &text[paren_start + 2..text.len() - 1];
            (&text[..paren_start], Some(crumb.to_string()))
        } else {
            (text, None)
        }
    } else {
        (text, None)
    };

    // Remove from finished
    let mut new_finished: Vec<&str> = finished_lines.clone();
    new_finished.remove(cursor_line);
    if new_finished.is_empty() {
        new_finished.push("");
    }

    // Insert into todo under matching parent, or at first gap
    let new_line = format!("- {clean_text}");

    let todo_result = if let Some(ref crumb) = breadcrumb {
        let mut todo_lines: Vec<String> = if todo_text.trim().is_empty() {
            Vec::new()
        } else {
            todo_text.lines().map(ToString::to_string).collect()
        };
        if let Some(pos) = find_parent_position(&todo_lines, crumb) {
            todo_lines.insert(pos, new_line);
            ensure_trailing_blank_line(todo_lines).join("\n")
        } else {
            insert_at_first_gap(todo_text, &new_line)
        }
    } else {
        insert_at_first_gap(todo_text, &new_line)
    };

    Some((new_finished.join("\n"), todo_result))
}

/// Fill empty day headers between oldest existing date and today.
pub fn fill_empty_days(text: &str, today: NaiveDate, date_format: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();

    let mut dates: Vec<NaiveDate> = Vec::new();
    for line in &lines {
        if let Some(date_str) = line.strip_prefix("## ") {
            if let Some(date) = parse_date_flexible(date_str.trim()) {
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

    let mut result_lines: Vec<String> = lines.iter().map(ToString::to_string).collect();

    let mut date = oldest;
    while date <= newest {
        if !dates.contains(&date) {
            let formatted = date.format(date_format);
            let header = format!("## {formatted}");
            let pos = find_date_insert_position(&result_lines, date);
            result_lines.insert(pos, String::new());
            result_lines.insert(pos, header);
        }
        date += chrono::Duration::days(1);
    }

    result_lines.join("\n")
}

fn insert_into_finished(
    finished_text: &str,
    today: NaiveDate,
    entry: &str,
    date_format: &str,
) -> String {
    let mut lines: Vec<String> = if finished_text.trim().is_empty() {
        Vec::new()
    } else {
        finished_text.lines().map(ToString::to_string).collect()
    };

    let formatted = today.format(date_format);
    let date_header = format!("## {formatted}");

    // Find today's header — match by parsed date, not string, so format changes still work
    let header_idx = lines.iter().position(|l| {
        if let Some(ds) = l.strip_prefix("## ") {
            if let Some(d) = parse_date_flexible(ds.trim()) {
                return d == today;
            }
        }
        false
    });

    if let Some(idx) = header_idx {
        // Update header to current format
        lines[idx] = date_header;
        let mut insert_at = idx + 1;
        while insert_at < lines.len() {
            let trimmed = lines[insert_at].trim();
            if trimmed.is_empty() || trimmed.starts_with("## ") {
                break;
            }
            insert_at += 1;
        }
        lines.insert(insert_at, entry.to_string());
        // Ensure blank line after the inserted item (section separator)
        if insert_at + 1 >= lines.len() || !lines[insert_at + 1].trim().is_empty() {
            lines.insert(insert_at + 1, String::new());
        }
    } else if lines.is_empty() {
        lines.push(date_header);
        lines.push(entry.to_string());
        lines.push(String::new());
    } else {
        // Insert at top (newest first)
        lines.insert(0, String::new());
        lines.insert(0, entry.to_string());
        lines.insert(0, date_header);
    }

    ensure_trailing_blank_line(lines).join("\n")
}

fn find_date_insert_position(lines: &[String], date: NaiveDate) -> usize {
    for (i, line) in lines.iter().enumerate() {
        if let Some(date_str) = line.strip_prefix("## ") {
            if let Some(existing) = parse_date_flexible(date_str.trim()) {
                if date > existing {
                    return i;
                }
            }
        }
    }
    lines.len()
}

/// Rewrite all date headers in `text` to use `new_format`.
pub fn reformat_date_headers(text: &str, new_format: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut result: Vec<String> = Vec::with_capacity(lines.len());
    for line in &lines {
        if let Some(date_str) = line.strip_prefix("## ") {
            if let Some(date) = parse_date_flexible(date_str.trim()) {
                let formatted = date.format(new_format);
                result.push(format!("## {formatted}"));
                continue;
            }
        }
        result.push((*line).to_string());
    }
    result.join("\n")
}

/// Try parsing a date string in common formats.
fn parse_date_flexible(s: &str) -> Option<NaiveDate> {
    let formats = [
        "%Y-%m-%d",
        "%d-%m-%Y",
        "%d/%m/%Y",
        "%m/%d/%Y",
        "%B %d, %Y",
        "%d %B %Y",
    ];
    for fmt in &formats {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return Some(d);
        }
    }
    None
}

/// Insert an entry at the first available gap in the target text.
/// A "gap" is a blank line, a heading boundary, or the end of the first item group.
/// Always ensures a trailing blank line.
fn insert_at_first_gap(target_text: &str, entry: &str) -> String {
    let mut lines: Vec<String> = if target_text.trim().is_empty() {
        Vec::new()
    } else {
        target_text.lines().map(ToString::to_string).collect()
    };

    if lines.is_empty() {
        lines.push(entry.to_string());
        lines.push(String::new());
        return lines.join("\n");
    }

    // Walk the lines: find the first blank line or the first heading after
    // we've seen at least one item. This places the entry at the end of
    // the first section/group of items.
    let mut saw_item = false;
    let mut insert_pos = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("- ") {
            saw_item = true;
        } else if saw_item && (trimmed.is_empty() || trimmed.starts_with('#')) {
            // First gap or heading after the first group of items
            insert_pos = Some(i);
            break;
        }
    }

    let pos = insert_pos.unwrap_or(lines.len());
    lines.insert(pos, entry.to_string());

    ensure_trailing_blank_line(lines).join("\n")
}

/// Ensure the lines end with exactly one blank line.
fn ensure_trailing_blank_line(mut lines: Vec<String>) -> Vec<String> {
    // Remove trailing blank lines
    while lines.last().map_or(false, |l| l.trim().is_empty()) {
        lines.pop();
    }
    // Add exactly one
    lines.push(String::new());
    lines
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_at_first_gap() {
        // No blank lines → append after last item + trailing blank
        let result = insert_at_first_gap("- a\n- b", "- new");
        assert_eq!(result, "- a\n- b\n- new\n");

        // Blank line between items → insert at end of first group
        let result = insert_at_first_gap("- a\n\n- b", "- new");
        assert_eq!(result, "- a\n- new\n\n- b\n");

        // Heading after items → insert before heading
        let result = insert_at_first_gap("## H1\n- a\n## H2\n- b", "- new");
        assert_eq!(result, "## H1\n- a\n- new\n## H2\n- b\n");

        // Empty target
        let result = insert_at_first_gap("", "- new");
        assert_eq!(result, "- new\n");

        // Target with just newlines
        let result = insert_at_first_gap("\n\n", "- new");
        assert_eq!(result, "- new\n");
    }
}
