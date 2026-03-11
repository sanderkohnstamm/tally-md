use std::collections::HashSet;
use std::path::PathBuf;
use chrono::Local;
use crate::clipboard;
use crate::editor::Buffer;
use crate::finished;
use crate::keys::{Action, InputMode};
use crate::pane::PaneState;

#[derive(Debug, Clone, PartialEq)]
pub enum FocusPane {
    Todo,
    Finished,
}

pub struct App {
    pub todo_buf: Buffer,
    pub finished_buf: Buffer,
    pub focus: FocusPane,
    pub todo_pane: PaneState,
    pub finished_pane: PaneState,
    pub todo_path: PathBuf,
    pub finished_path: PathBuf,
    pub should_quit: bool,
    pub message: Option<String>,
    pub input_mode: InputMode,
    pub search_query: String,
    pub last_copied: Option<String>,
    pub folded: HashSet<usize>,
}

impl App {
    pub fn new(
        todo_buf: Buffer,
        finished_buf: Buffer,
        todo_path: PathBuf,
        finished_path: PathBuf,
    ) -> Self {
        Self {
            todo_buf,
            finished_buf,
            focus: FocusPane::Todo,
            todo_pane: PaneState::new(),
            finished_pane: PaneState { scroll: 0, editing: false },
            todo_path,
            finished_path,
            should_quit: false,
            message: None,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            last_copied: None,
            folded: HashSet::new(),
        }
    }

    fn active_buf(&self) -> &Buffer {
        match self.focus {
            FocusPane::Todo => &self.todo_buf,
            FocusPane::Finished => &self.finished_buf,
        }
    }

    fn active_buf_mut(&mut self) -> &mut Buffer {
        match self.focus {
            FocusPane::Todo => &mut self.todo_buf,
            FocusPane::Finished => &mut self.finished_buf,
        }
    }

    pub fn active_pane(&self) -> &PaneState {
        match self.focus {
            FocusPane::Todo => &self.todo_pane,
            FocusPane::Finished => &self.finished_pane,
        }
    }

    fn active_pane_mut(&mut self) -> &mut PaneState {
        match self.focus {
            FocusPane::Todo => &mut self.todo_pane,
            FocusPane::Finished => &mut self.finished_pane,
        }
    }

    fn is_editing(&self) -> bool {
        self.active_pane().editing
    }

    pub fn save(&mut self) {
        // Save todo
        let content = self.todo_buf.to_string();
        if let Some(parent) = self.todo_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&self.todo_path, &content);
        self.todo_buf.dirty = false;

        // Save finished
        let content = self.finished_buf.to_string();
        let _ = std::fs::write(&self.finished_path, &content);
        self.finished_buf.dirty = false;

        // Switch to preview on save
        self.todo_pane.editing = false;
        self.finished_pane.editing = false;

        self.message = Some("Saved".to_string());
    }

    pub fn update(&mut self, action: Action, page_size: usize) {
        self.message = None;

        match action {
            Action::Quit => {
                if self.input_mode == InputMode::Search {
                    self.input_mode = InputMode::Normal;
                    self.search_query.clear();
                } else {
                    self.should_quit = true;
                }
            }

            // --- Navigation ---
            Action::CursorUp => self.active_buf_mut().move_up(false),
            Action::CursorDown => self.active_buf_mut().move_down(false),
            Action::CursorLeft => {
                if self.is_editing() {
                    self.active_buf_mut().move_left(false);
                }
            }
            Action::CursorRight => {
                if self.is_editing() {
                    self.active_buf_mut().move_right(false);
                }
            }
            Action::CursorWordLeft => self.active_buf_mut().move_word_left(false),
            Action::CursorWordRight => self.active_buf_mut().move_word_right(false),
            Action::CursorHome => self.active_buf_mut().move_home(false),
            Action::CursorEnd => self.active_buf_mut().move_end(false),
            Action::CursorPageUp => self.active_buf_mut().move_page_up(page_size, false),
            Action::CursorPageDown => self.active_buf_mut().move_page_down(page_size, false),
            Action::CursorDocStart => self.active_buf_mut().move_doc_start(false),
            Action::CursorDocEnd => self.active_buf_mut().move_doc_end(false),

            // --- Selection ---
            Action::SelectUp => self.active_buf_mut().move_up(true),
            Action::SelectDown => self.active_buf_mut().move_down(true),
            Action::SelectLeft => self.active_buf_mut().move_left(true),
            Action::SelectRight => self.active_buf_mut().move_right(true),
            Action::SelectWordLeft => self.active_buf_mut().move_word_left(true),
            Action::SelectWordRight => self.active_buf_mut().move_word_right(true),
            Action::SelectHome => self.active_buf_mut().move_home(true),
            Action::SelectEnd => self.active_buf_mut().move_end(true),
            Action::SelectPageUp => self.active_buf_mut().move_page_up(page_size, true),
            Action::SelectPageDown => self.active_buf_mut().move_page_down(page_size, true),
            Action::SelectAll => self.active_buf_mut().select_all(),

            // --- Editing (only in edit mode) ---
            Action::InsertChar(c) => {
                if self.is_editing() {
                    self.active_buf_mut().insert_char(c);
                }
            }
            Action::Backspace => {
                if self.is_editing() {
                    self.active_buf_mut().backspace();
                }
            }
            Action::DeleteChar => {
                if self.is_editing() {
                    self.active_buf_mut().delete();
                }
            }
            Action::DeleteWord => {
                if self.is_editing() {
                    self.active_buf_mut().delete_word_back();
                }
            }
            Action::Enter => {
                if self.is_editing() {
                    self.active_buf_mut().enter();
                }
            }
            Action::IndentRight => {
                if self.is_editing() {
                    self.active_buf_mut().indent_line();
                }
            }
            Action::IndentLeft => {
                if self.is_editing() {
                    self.active_buf_mut().outdent_line();
                }
            }
            Action::DeleteLine => {
                if self.is_editing() {
                    self.active_buf_mut().delete_line();
                }
            }
            Action::Undo => self.active_buf_mut().undo(),
            Action::Redo => self.active_buf_mut().redo(),

            // --- Clipboard ---
            Action::Cut => {
                let text = if self.active_buf().selection.is_some() {
                    self.active_buf_mut().delete_selection()
                } else if self.is_editing() {
                    // Cut whole line as text
                    let line = self.active_buf().lines[self.active_buf().cursor.line].clone();
                    self.active_buf_mut().delete_line();
                    Some(line)
                } else {
                    None
                };
                if let Some(t) = text {
                    clipboard::copy_to_system(&t);
                    self.last_copied = Some(t);
                    self.message = Some("Cut".to_string());
                }
            }
            Action::Copy => {
                let text = if let Some(t) = self.active_buf().selected_text() {
                    Some(t)
                } else {
                    // Copy whole line
                    Some(self.active_buf().lines[self.active_buf().cursor.line].clone())
                };
                if let Some(t) = text {
                    clipboard::copy_to_system(&t);
                    self.last_copied = Some(t);
                    self.message = Some("Copied".to_string());
                }
            }
            Action::Paste => {
                if !self.is_editing() {
                    return;
                }
                // Check system clipboard
                let sys = clipboard::paste_from_system();
                let text = if sys.as_ref().map_or(false, |s| {
                    self.last_copied.as_ref().map_or(true, |lc| lc != s)
                }) {
                    sys
                } else {
                    self.last_copied.clone()
                };
                if let Some(t) = text {
                    self.active_buf_mut().insert_str(&t);
                    self.message = Some("Pasted".to_string());
                } else {
                    self.message = Some("Nothing to paste".to_string());
                }
            }

            // --- Commands ---
            Action::Save => self.save(),

            Action::Complete => {
                match self.focus {
                    FocusPane::Todo => {
                        let today = Local::now().date_naive();
                        if finished::complete_item(&mut self.todo_buf, &mut self.finished_buf, today) {
                            self.message = Some("Completed!".to_string());
                        } else {
                            self.message = Some("Not a list item (- )".to_string());
                        }
                    }
                    FocusPane::Finished => {
                        // Recover item back to todo
                        if finished::recover_item(&mut self.finished_buf, &mut self.todo_buf) {
                            self.message = Some("Recovered to todo".to_string());
                        } else {
                            self.message = Some("Not a list item".to_string());
                        }
                    }
                }
            }

            Action::ToggleFold => {
                let line = self.active_buf().cursor.line;
                if self.folded.contains(&line) {
                    self.folded.remove(&line);
                } else {
                    // Only fold headings
                    let kind = crate::markdown::classify_line(
                        &self.active_buf().lines[line], false
                    );
                    if matches!(kind, crate::markdown::LineKind::Heading(_)) {
                        self.folded.insert(line);
                    }
                }
            }

            Action::ToggleEdit => {
                let pane = self.active_pane_mut();
                pane.editing = !pane.editing;
                if pane.editing {
                    self.message = Some("Edit mode".to_string());
                } else {
                    self.message = Some("Preview mode".to_string());
                }
            }

            Action::SwitchPane => {
                self.focus = match self.focus {
                    FocusPane::Todo => FocusPane::Finished,
                    FocusPane::Finished => FocusPane::Todo,
                };
            }

            // --- Search ---
            Action::Search => {
                self.input_mode = InputMode::Search;
                self.search_query.clear();
                self.message = Some("Search:".to_string());
            }
            Action::SearchInsertChar(c) => {
                self.search_query.push(c);
                // Live search: jump to first match
                let query = self.search_query.clone();
                let pos = self.active_buf().cursor;
                if let Some(found) = self.active_buf().find_next(&query, pos) {
                    self.active_buf_mut().cursor = found;
                    self.active_buf_mut().clear_selection();
                }
            }
            Action::SearchBackspace => {
                self.search_query.pop();
            }
            Action::SearchConfirm | Action::SearchNext => {
                let query = self.search_query.clone();
                let mut from = self.active_buf().cursor;
                from.col += 1; // skip current match
                if let Some(found) = self.active_buf().find_next(&query, from) {
                    self.active_buf_mut().cursor = found;
                    self.active_buf_mut().clear_selection();
                }
                if matches!(action, Action::SearchConfirm) {
                    self.input_mode = InputMode::Normal;
                }
            }
            Action::SearchPrev => {
                let query = self.search_query.clone();
                let from = self.active_buf().cursor;
                if let Some(found) = self.active_buf().find_prev(&query, from) {
                    self.active_buf_mut().cursor = found;
                    self.active_buf_mut().clear_selection();
                }
            }
            Action::SearchCancel => {
                self.input_mode = InputMode::Normal;
                self.search_query.clear();
            }

            // --- Mouse ---
            Action::MouseClick(x, y) => {
                self.handle_mouse_click(x, y);
            }
            Action::MouseDrag(x, y) => {
                self.handle_mouse_drag(x, y);
            }
            Action::MouseScroll(delta, _x, y) => {
                self.handle_mouse_scroll(delta, y);
            }

            Action::Noop => {}
        }
    }

    fn handle_mouse_click(&mut self, x: u16, y: u16) {
        let scroll = match self.focus {
            FocusPane::Todo => self.todo_pane.scroll,
            FocusPane::Finished => self.finished_pane.scroll,
        };
        let line = scroll + y.saturating_sub(1) as usize;
        let buf = self.active_buf_mut();
        if line < buf.lines.len() {
            let col = x.saturating_sub(1) as usize;
            let max_col = buf.lines[line].chars().count();
            buf.cursor.line = line;
            buf.cursor.col = col.min(max_col);
            buf.clear_selection();
        }
    }

    fn handle_mouse_drag(&mut self, x: u16, y: u16) {
        let scroll = match self.focus {
            FocusPane::Todo => self.todo_pane.scroll,
            FocusPane::Finished => self.finished_pane.scroll,
        };
        let line = scroll + y.saturating_sub(1) as usize;
        let buf = self.active_buf_mut();
        if line < buf.lines.len() {
            buf.start_or_extend_selection();
            let col = x.saturating_sub(1) as usize;
            let max_col = buf.lines[line].chars().count();
            buf.cursor.line = line;
            buf.cursor.col = col.min(max_col);
            buf.update_selection_cursor();
        }
    }

    fn handle_mouse_scroll(&mut self, delta: i32, _y: u16) {
        let pane = self.active_pane_mut();
        if delta > 0 {
            pane.scroll = pane.scroll.saturating_add(delta as usize);
        } else {
            pane.scroll = pane.scroll.saturating_sub((-delta) as usize);
        }
    }
}
