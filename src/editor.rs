/// Core line-based text buffer with cursor, selection, and undo/redo.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub line: usize,
    pub col: usize, // char offset
}

impl Pos {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
    pub fn zero() -> Self {
        Self { line: 0, col: 0 }
    }
}

impl PartialOrd for Pos {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Pos {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line.cmp(&other.line).then(self.col.cmp(&other.col))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Pos,
    pub cursor: Pos,
}

impl Selection {
    pub fn ordered(&self) -> (Pos, Pos) {
        if self.anchor <= self.cursor {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        }
    }
}

#[derive(Debug, Clone)]
struct Snapshot {
    lines: Vec<String>,
    cursor: Pos,
}

pub struct Buffer {
    pub lines: Vec<String>,
    pub cursor: Pos,
    pub selection: Option<Selection>,
    pub desired_col: Option<usize>,
    pub dirty: bool,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    last_snapshot_time: std::time::Instant,
}

impl Buffer {
    pub fn from_string(content: &str) -> Self {
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(|l| l.to_string()).collect()
        };
        // Ensure at least one line
        let lines = if lines.is_empty() { vec![String::new()] } else { lines };
        Self {
            lines,
            cursor: Pos::zero(),
            selection: None,
            desired_col: None,
            dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_snapshot_time: std::time::Instant::now(),
        }
    }

    pub fn to_string(&self) -> String {
        self.lines.join("\n")
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    fn char_len(&self, line: usize) -> usize {
        self.lines.get(line).map(|l| l.chars().count()).unwrap_or(0)
    }

    fn char_to_byte(s: &str, char_pos: usize) -> usize {
        s.char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(s.len())
    }

    fn clamp_cursor(&mut self) {
        if self.cursor.line >= self.lines.len() {
            self.cursor.line = self.lines.len() - 1;
        }
        let len = self.char_len(self.cursor.line);
        if self.cursor.col > len {
            self.cursor.col = len;
        }
    }

    fn push_undo(&mut self) {
        let now = std::time::Instant::now();
        // Batch: don't snapshot if last one was <300ms ago
        if now.duration_since(self.last_snapshot_time).as_millis() < 300
            && !self.undo_stack.is_empty()
        {
            // Update the top snapshot's cursor only
            if let Some(top) = self.undo_stack.last_mut() {
                top.cursor = self.cursor;
            }
        } else {
            self.undo_stack.push(Snapshot {
                lines: self.lines.clone(),
                cursor: self.cursor,
            });
        }
        self.redo_stack.clear();
        self.last_snapshot_time = now;
        self.dirty = true;
    }

    pub fn force_undo_snapshot(&mut self) {
        self.undo_stack.push(Snapshot {
            lines: self.lines.clone(),
            cursor: self.cursor,
        });
        self.redo_stack.clear();
        self.last_snapshot_time = std::time::Instant::now();
        self.dirty = true;
    }

    pub fn undo(&mut self) {
        if let Some(snap) = self.undo_stack.pop() {
            self.redo_stack.push(Snapshot {
                lines: self.lines.clone(),
                cursor: self.cursor,
            });
            self.lines = snap.lines;
            self.cursor = snap.cursor;
            self.selection = None;
            self.clamp_cursor();
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(snap) = self.redo_stack.pop() {
            self.undo_stack.push(Snapshot {
                lines: self.lines.clone(),
                cursor: self.cursor,
            });
            self.lines = snap.lines;
            self.cursor = snap.cursor;
            self.selection = None;
            self.clamp_cursor();
            self.dirty = true;
        }
    }

    // --- Selection helpers ---

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn start_or_extend_selection(&mut self) {
        if self.selection.is_none() {
            self.selection = Some(Selection {
                anchor: self.cursor,
                cursor: self.cursor,
            });
        }
    }

    pub fn update_selection_cursor(&mut self) {
        if let Some(ref mut sel) = self.selection {
            sel.cursor = self.cursor;
        }
    }

    pub fn selected_text(&self) -> Option<String> {
        let sel = self.selection?;
        let (start, end) = sel.ordered();
        if start == end {
            return None;
        }
        if start.line == end.line {
            let line = &self.lines[start.line];
            let bs = Self::char_to_byte(line, start.col);
            let be = Self::char_to_byte(line, end.col);
            return Some(line[bs..be].to_string());
        }
        let mut result = String::new();
        // First line
        let first = &self.lines[start.line];
        let bs = Self::char_to_byte(first, start.col);
        result.push_str(&first[bs..]);
        result.push('\n');
        // Middle lines
        for i in (start.line + 1)..end.line {
            result.push_str(&self.lines[i]);
            result.push('\n');
        }
        // Last line
        let last = &self.lines[end.line];
        let be = Self::char_to_byte(last, end.col);
        result.push_str(&last[..be]);
        Some(result)
    }

    pub fn delete_selection(&mut self) -> Option<String> {
        let text = self.selected_text()?;
        let sel = self.selection?;
        let (start, end) = sel.ordered();

        self.force_undo_snapshot();

        if start.line == end.line {
            let line = &mut self.lines[start.line];
            let bs = Self::char_to_byte(line, start.col);
            let be = Self::char_to_byte(line, end.col);
            line.replace_range(bs..be, "");
        } else {
            let first = &self.lines[start.line];
            let bs = Self::char_to_byte(first, start.col);
            let last = &self.lines[end.line];
            let be = Self::char_to_byte(last, end.col);
            let new_line = format!("{}{}", &first[..bs], &last[be..]);
            self.lines[start.line] = new_line;
            self.lines.drain((start.line + 1)..=end.line);
        }

        self.cursor = start;
        self.selection = None;
        self.dirty = true;
        Some(text)
    }

    pub fn select_all(&mut self) {
        let last_line = self.lines.len() - 1;
        let last_col = self.char_len(last_line);
        self.selection = Some(Selection {
            anchor: Pos::zero(),
            cursor: Pos::new(last_line, last_col),
        });
        self.cursor = Pos::new(last_line, last_col);
    }

    // --- Movement ---

    pub fn move_left(&mut self, extend: bool) {
        if extend {
            self.start_or_extend_selection();
        } else {
            self.clear_selection();
        }
        self.desired_col = None;
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.col = self.char_len(self.cursor.line);
        }
        if extend {
            self.update_selection_cursor();
        }
    }

    pub fn move_right(&mut self, extend: bool) {
        if extend {
            self.start_or_extend_selection();
        } else {
            self.clear_selection();
        }
        self.desired_col = None;
        let len = self.char_len(self.cursor.line);
        if self.cursor.col < len {
            self.cursor.col += 1;
        } else if self.cursor.line + 1 < self.lines.len() {
            self.cursor.line += 1;
            self.cursor.col = 0;
        }
        if extend {
            self.update_selection_cursor();
        }
    }

    pub fn move_up(&mut self, extend: bool) {
        if extend {
            self.start_or_extend_selection();
        } else {
            self.clear_selection();
        }
        if self.cursor.line > 0 {
            let dc = self.desired_col.unwrap_or(self.cursor.col);
            self.cursor.line -= 1;
            self.cursor.col = dc.min(self.char_len(self.cursor.line));
            self.desired_col = Some(dc);
        }
        if extend {
            self.update_selection_cursor();
        }
    }

    pub fn move_down(&mut self, extend: bool) {
        if extend {
            self.start_or_extend_selection();
        } else {
            self.clear_selection();
        }
        if self.cursor.line + 1 < self.lines.len() {
            let dc = self.desired_col.unwrap_or(self.cursor.col);
            self.cursor.line += 1;
            self.cursor.col = dc.min(self.char_len(self.cursor.line));
            self.desired_col = Some(dc);
        }
        if extend {
            self.update_selection_cursor();
        }
    }

    pub fn move_word_left(&mut self, extend: bool) {
        if extend {
            self.start_or_extend_selection();
        } else {
            self.clear_selection();
        }
        self.desired_col = None;
        if self.cursor.col == 0 {
            if self.cursor.line > 0 {
                self.cursor.line -= 1;
                self.cursor.col = self.char_len(self.cursor.line);
            }
        } else {
            let chars: Vec<char> = self.lines[self.cursor.line].chars().collect();
            let mut pos = self.cursor.col;
            while pos > 0 && chars[pos - 1] == ' ' {
                pos -= 1;
            }
            while pos > 0 && chars[pos - 1] != ' ' {
                pos -= 1;
            }
            self.cursor.col = pos;
        }
        if extend {
            self.update_selection_cursor();
        }
    }

    pub fn move_word_right(&mut self, extend: bool) {
        if extend {
            self.start_or_extend_selection();
        } else {
            self.clear_selection();
        }
        self.desired_col = None;
        let len = self.char_len(self.cursor.line);
        if self.cursor.col >= len {
            if self.cursor.line + 1 < self.lines.len() {
                self.cursor.line += 1;
                self.cursor.col = 0;
            }
        } else {
            let chars: Vec<char> = self.lines[self.cursor.line].chars().collect();
            let mut pos = self.cursor.col;
            while pos < chars.len() && chars[pos] != ' ' {
                pos += 1;
            }
            while pos < chars.len() && chars[pos] == ' ' {
                pos += 1;
            }
            self.cursor.col = pos;
        }
        if extend {
            self.update_selection_cursor();
        }
    }

    pub fn move_home(&mut self, extend: bool) {
        if extend {
            self.start_or_extend_selection();
        } else {
            self.clear_selection();
        }
        self.desired_col = None;
        // Smart home: go to first non-space, or col 0 if already there
        let chars: Vec<char> = self.lines[self.cursor.line].chars().collect();
        let first_non_space = chars.iter().position(|c| *c != ' ').unwrap_or(0);
        self.cursor.col = if self.cursor.col == first_non_space {
            0
        } else {
            first_non_space
        };
        if extend {
            self.update_selection_cursor();
        }
    }

    pub fn move_end(&mut self, extend: bool) {
        if extend {
            self.start_or_extend_selection();
        } else {
            self.clear_selection();
        }
        self.desired_col = None;
        self.cursor.col = self.char_len(self.cursor.line);
        if extend {
            self.update_selection_cursor();
        }
    }

    pub fn move_page_up(&mut self, page_size: usize, extend: bool) {
        if extend {
            self.start_or_extend_selection();
        } else {
            self.clear_selection();
        }
        let dc = self.desired_col.unwrap_or(self.cursor.col);
        self.cursor.line = self.cursor.line.saturating_sub(page_size);
        self.cursor.col = dc.min(self.char_len(self.cursor.line));
        self.desired_col = Some(dc);
        if extend {
            self.update_selection_cursor();
        }
    }

    pub fn move_page_down(&mut self, page_size: usize, extend: bool) {
        if extend {
            self.start_or_extend_selection();
        } else {
            self.clear_selection();
        }
        let dc = self.desired_col.unwrap_or(self.cursor.col);
        self.cursor.line = (self.cursor.line + page_size).min(self.lines.len() - 1);
        self.cursor.col = dc.min(self.char_len(self.cursor.line));
        self.desired_col = Some(dc);
        if extend {
            self.update_selection_cursor();
        }
    }

    pub fn move_doc_start(&mut self, extend: bool) {
        if extend {
            self.start_or_extend_selection();
        } else {
            self.clear_selection();
        }
        self.desired_col = None;
        self.cursor = Pos::zero();
        if extend {
            self.update_selection_cursor();
        }
    }

    pub fn move_doc_end(&mut self, extend: bool) {
        if extend {
            self.start_or_extend_selection();
        } else {
            self.clear_selection();
        }
        self.desired_col = None;
        let last = self.lines.len() - 1;
        self.cursor = Pos::new(last, self.char_len(last));
        if extend {
            self.update_selection_cursor();
        }
    }

    // --- Editing ---

    pub fn insert_char(&mut self, c: char) {
        if self.selection.is_some() {
            self.delete_selection();
        }
        self.push_undo();
        let line = &mut self.lines[self.cursor.line];
        let byte_pos = Self::char_to_byte(line, self.cursor.col);
        line.insert(byte_pos, c);
        self.cursor.col += 1;
        self.desired_col = None;
    }

    pub fn insert_str(&mut self, s: &str) {
        if self.selection.is_some() {
            self.delete_selection();
        }
        self.force_undo_snapshot();

        let insert_lines: Vec<&str> = s.split('\n').collect();
        if insert_lines.len() == 1 {
            let line = &mut self.lines[self.cursor.line];
            let byte_pos = Self::char_to_byte(line, self.cursor.col);
            line.insert_str(byte_pos, insert_lines[0]);
            self.cursor.col += insert_lines[0].chars().count();
        } else {
            let current = &self.lines[self.cursor.line];
            let byte_pos = Self::char_to_byte(current, self.cursor.col);
            let after = current[byte_pos..].to_string();
            let before = current[..byte_pos].to_string();

            // First fragment
            self.lines[self.cursor.line] = format!("{}{}", before, insert_lines[0]);
            // Middle lines
            let insert_at = self.cursor.line + 1;
            for (i, frag) in insert_lines[1..insert_lines.len() - 1].iter().enumerate() {
                self.lines.insert(insert_at + i, frag.to_string());
            }
            // Last fragment + remaining text
            let last_frag = insert_lines.last().unwrap();
            let last_idx = self.cursor.line + insert_lines.len() - 1;
            let new_last = format!("{}{}", last_frag, after);
            if last_idx < self.lines.len() {
                self.lines.insert(last_idx, new_last);
            } else {
                self.lines.push(new_last);
            }
            self.cursor.line = last_idx;
            self.cursor.col = last_frag.chars().count();
        }
        self.desired_col = None;
        self.dirty = true;
    }

    pub fn backspace(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }
        if self.cursor.col > 0 {
            self.push_undo();
            let line = &mut self.lines[self.cursor.line];
            let byte_pos = Self::char_to_byte(line, self.cursor.col - 1);
            line.remove(byte_pos);
            self.cursor.col -= 1;
        } else if self.cursor.line > 0 {
            // Join with previous line
            self.push_undo();
            let current = self.lines.remove(self.cursor.line);
            self.cursor.line -= 1;
            self.cursor.col = self.char_len(self.cursor.line);
            self.lines[self.cursor.line].push_str(&current);
        }
        self.desired_col = None;
    }

    pub fn delete(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }
        let len = self.char_len(self.cursor.line);
        if self.cursor.col < len {
            self.push_undo();
            let line = &mut self.lines[self.cursor.line];
            let byte_pos = Self::char_to_byte(line, self.cursor.col);
            line.remove(byte_pos);
        } else if self.cursor.line + 1 < self.lines.len() {
            // Join with next line
            self.push_undo();
            let next = self.lines.remove(self.cursor.line + 1);
            self.lines[self.cursor.line].push_str(&next);
        }
    }

    pub fn delete_word_back(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }
        let target = {
            let chars: Vec<char> = self.lines[self.cursor.line].chars().collect();
            let mut pos = self.cursor.col;
            while pos > 0 && chars[pos - 1] == ' ' {
                pos -= 1;
            }
            while pos > 0 && chars[pos - 1] != ' ' {
                pos -= 1;
            }
            pos
        };
        if target < self.cursor.col {
            self.force_undo_snapshot();
            let line = &mut self.lines[self.cursor.line];
            let bs = Self::char_to_byte(line, target);
            let be = Self::char_to_byte(line, self.cursor.col);
            line.replace_range(bs..be, "");
            self.cursor.col = target;
        }
        self.desired_col = None;
    }

    pub fn enter(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
        }
        self.force_undo_snapshot();

        let line = &self.lines[self.cursor.line];
        let byte_pos = Self::char_to_byte(line, self.cursor.col);

        // Smart enter: detect list context and auto-prefix
        let prefix = list_continuation_prefix(line);
        let after = line[byte_pos..].to_string();
        self.lines[self.cursor.line] = line[..byte_pos].to_string();

        let current_text = &self.lines[self.cursor.line];
        let new_line = if let Some(pfx) = prefix {
            // If the current line is just the bullet prefix (e.g. "- " with nothing after),
            // then pressing enter should end the list with a blank line.
            // Otherwise, continue the list prefix on the new line.
            let current_after_prefix = current_text.trim_start();
            let is_empty_item = current_after_prefix == "- " || current_after_prefix == "-";
            if is_empty_item && after.trim().is_empty() {
                // Clear the empty bullet from current line too
                self.lines[self.cursor.line] = String::new();
                String::new()
            } else {
                format!("{}{}", pfx, after)
            }
        } else {
            after
        };

        let new_col = if let Some(ref pfx) = list_continuation_prefix(&new_line) {
            pfx.chars().count()
        } else {
            0
        };

        self.lines.insert(self.cursor.line + 1, new_line);
        self.cursor.line += 1;
        self.cursor.col = new_col;
        self.desired_col = None;
    }

    pub fn delete_line(&mut self) {
        self.force_undo_snapshot();
        self.selection = None;
        if self.lines.len() > 1 {
            self.lines.remove(self.cursor.line);
            if self.cursor.line >= self.lines.len() {
                self.cursor.line = self.lines.len() - 1;
            }
            self.cursor.col = self.cursor.col.min(self.char_len(self.cursor.line));
        } else {
            self.lines[0] = String::new();
            self.cursor.col = 0;
        }
        self.desired_col = None;
    }

    pub fn indent_line(&mut self) {
        self.push_undo();
        self.lines[self.cursor.line].insert_str(0, "  ");
        self.cursor.col += 2;
        self.desired_col = None;
    }

    pub fn outdent_line(&mut self) {
        let line = &self.lines[self.cursor.line];
        let spaces = line.chars().take_while(|c| *c == ' ').count().min(2);
        if spaces > 0 {
            self.push_undo();
            self.lines[self.cursor.line] = self.lines[self.cursor.line][spaces..].to_string();
            self.cursor.col = self.cursor.col.saturating_sub(spaces);
            self.desired_col = None;
        }
    }

    // --- Search ---

    pub fn find_next(&self, query: &str, from: Pos) -> Option<Pos> {
        if query.is_empty() {
            return None;
        }
        let query_lower = query.to_lowercase();
        // Search from `from` to end, then wrap
        for line_idx in from.line..self.lines.len() {
            let line_lower = self.lines[line_idx].to_lowercase();
            let start_col = if line_idx == from.line { from.col } else { 0 };
            let byte_start = Self::char_to_byte(&line_lower, start_col);
            if let Some(byte_offset) = line_lower[byte_start..].find(&query_lower) {
                let char_col = line_lower[..byte_start + byte_offset].chars().count();
                return Some(Pos::new(line_idx, char_col));
            }
        }
        // Wrap around
        for line_idx in 0..=from.line.min(self.lines.len() - 1) {
            let line_lower = self.lines[line_idx].to_lowercase();
            if let Some(byte_offset) = line_lower.find(&query_lower) {
                let char_col = line_lower[..byte_offset].chars().count();
                return Some(Pos::new(line_idx, char_col));
            }
        }
        None
    }

    pub fn find_prev(&self, query: &str, from: Pos) -> Option<Pos> {
        if query.is_empty() {
            return None;
        }
        let query_lower = query.to_lowercase();
        // Search backward
        for line_idx in (0..=from.line).rev() {
            let line_lower = self.lines[line_idx].to_lowercase();
            let end_col = if line_idx == from.line {
                Self::char_to_byte(&line_lower, from.col)
            } else {
                line_lower.len()
            };
            if let Some(byte_offset) = line_lower[..end_col].rfind(&query_lower) {
                let char_col = line_lower[..byte_offset].chars().count();
                return Some(Pos::new(line_idx, char_col));
            }
        }
        // Wrap around
        for line_idx in (from.line..self.lines.len()).rev() {
            let line_lower = self.lines[line_idx].to_lowercase();
            if let Some(byte_offset) = line_lower.rfind(&query_lower) {
                let char_col = line_lower[..byte_offset].chars().count();
                return Some(Pos::new(line_idx, char_col));
            }
        }
        None
    }
}

/// Given a line, return the prefix to use for a new list item continuation.
/// e.g., "  - foo" → Some("  - "), "not a list" → None
fn list_continuation_prefix(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("- ") {
        let indent = line.len() - trimmed.len();
        Some(format!("{}- ", " ".repeat(indent)))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_editing() {
        let mut buf = Buffer::from_string("hello world");
        assert_eq!(buf.lines, vec!["hello world"]);
        buf.cursor = Pos::new(0, 5);
        buf.insert_char('!');
        assert_eq!(buf.lines[0], "hello! world");
    }

    #[test]
    fn test_enter_splits_line() {
        let mut buf = Buffer::from_string("hello world");
        buf.cursor = Pos::new(0, 5);
        buf.enter();
        assert_eq!(buf.lines, vec!["hello", " world"]);
    }

    #[test]
    fn test_backspace_joins_lines() {
        let mut buf = Buffer::from_string("hello\nworld");
        buf.cursor = Pos::new(1, 0);
        buf.backspace();
        assert_eq!(buf.lines, vec!["helloworld"]);
        assert_eq!(buf.cursor, Pos::new(0, 5));
    }

    #[test]
    fn test_multiline_selection() {
        let mut buf = Buffer::from_string("aaa\nbbb\nccc");
        buf.selection = Some(Selection {
            anchor: Pos::new(0, 1),
            cursor: Pos::new(2, 2),
        });
        buf.cursor = Pos::new(2, 2);
        let text = buf.selected_text().unwrap();
        assert_eq!(text, "aa\nbbb\ncc");
    }

    #[test]
    fn test_delete_multiline_selection() {
        let mut buf = Buffer::from_string("aaa\nbbb\nccc");
        buf.selection = Some(Selection {
            anchor: Pos::new(0, 1),
            cursor: Pos::new(2, 2),
        });
        buf.cursor = Pos::new(2, 2);
        buf.delete_selection();
        assert_eq!(buf.lines, vec!["ac"]);
        assert_eq!(buf.cursor, Pos::new(0, 1));
    }

    #[test]
    fn test_smart_enter_list() {
        let mut buf = Buffer::from_string("- hello");
        buf.cursor = Pos::new(0, 7);
        buf.enter();
        assert_eq!(buf.lines, vec!["- hello", "- "]);
        assert_eq!(buf.cursor.col, 2);
    }

    #[test]
    fn test_undo_redo() {
        let mut buf = Buffer::from_string("hello");
        buf.cursor = Pos::new(0, 5);
        buf.force_undo_snapshot();
        buf.insert_char('!');
        assert_eq!(buf.lines[0], "hello!");
        buf.undo();
        assert_eq!(buf.lines[0], "hello");
        buf.redo();
        assert_eq!(buf.lines[0], "hello!");
    }
}
