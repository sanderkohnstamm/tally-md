use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    // Cursor movement
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    CursorWordLeft,
    CursorWordRight,
    CursorHome,
    CursorEnd,
    CursorPageUp,
    CursorPageDown,
    CursorDocStart,
    CursorDocEnd,
    // Selection (same as cursor but extend)
    SelectUp,
    SelectDown,
    SelectLeft,
    SelectRight,
    SelectWordLeft,
    SelectWordRight,
    SelectHome,
    SelectEnd,
    SelectPageUp,
    SelectPageDown,
    SelectAll,
    // Editing
    InsertChar(char),
    Backspace,
    DeleteChar,
    DeleteWord,
    Enter,
    IndentRight,
    IndentLeft,
    DeleteLine,
    // Clipboard
    Cut,
    Copy,
    Paste,
    // Commands
    Save,
    Complete, // Ctrl+Enter — complete a todo item
    ToggleFold,
    Undo,
    Redo,
    Search,
    SearchNext,
    SearchPrev,
    SwitchPane,
    ToggleEdit, // Switch between edit and preview mode
    Quit,
    // Search input
    SearchInsertChar(char),
    SearchBackspace,
    SearchConfirm,
    SearchCancel,
    // Mouse
    MouseClick(u16, u16),
    MouseDrag(u16, u16),
    MouseScroll(i32, u16, u16), // delta, x, y
    Noop,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
}

pub fn map_key(event: KeyEvent, mode: &InputMode) -> Action {
    match mode {
        InputMode::Normal => map_normal(event),
        InputMode::Search => map_search(event),
    }
}

pub fn map_mouse(event: MouseEvent) -> Action {
    match event.kind {
        MouseEventKind::Down(_) => Action::MouseClick(event.column, event.row),
        MouseEventKind::Drag(_) => Action::MouseDrag(event.column, event.row),
        MouseEventKind::ScrollUp => Action::MouseScroll(-3, event.column, event.row),
        MouseEventKind::ScrollDown => Action::MouseScroll(3, event.column, event.row),
        _ => Action::Noop,
    }
}

fn map_normal(event: KeyEvent) -> Action {
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    let shift = event.modifiers.contains(KeyModifiers::SHIFT);

    if ctrl && shift {
        return match event.code {
            KeyCode::Left => Action::SelectWordLeft,
            KeyCode::Right => Action::SelectWordRight,
            KeyCode::Char('z') | KeyCode::Char('Z') => Action::Redo,
            _ => Action::Noop,
        };
    }

    if ctrl {
        return match event.code {
            KeyCode::Char('s') => Action::Save,
            KeyCode::Char('d') => Action::DeleteLine,
            KeyCode::Char('f') => Action::Search,
            KeyCode::Char('x') => Action::Cut,
            KeyCode::Char('c') => Action::Copy,
            KeyCode::Char('v') => Action::Paste,
            KeyCode::Char('a') => Action::SelectAll,
            KeyCode::Char('z') => Action::Undo,
            KeyCode::Char('y') => Action::Redo,
            KeyCode::Char('q') => Action::Quit,
            KeyCode::Char('e') => Action::ToggleEdit,
            KeyCode::Enter => Action::Complete,
            KeyCode::Left => Action::CursorWordLeft,
            KeyCode::Right => Action::CursorWordRight,
            KeyCode::Home => Action::CursorDocStart,
            KeyCode::End => Action::CursorDocEnd,
            KeyCode::Backspace => Action::DeleteWord,
            KeyCode::Tab => Action::SwitchPane,
            _ => Action::Noop,
        };
    }

    if shift {
        return match event.code {
            KeyCode::Left => Action::SelectLeft,
            KeyCode::Right => Action::SelectRight,
            KeyCode::Up => Action::SelectUp,
            KeyCode::Down => Action::SelectDown,
            KeyCode::Home => Action::SelectHome,
            KeyCode::End => Action::SelectEnd,
            KeyCode::PageUp => Action::SelectPageUp,
            KeyCode::PageDown => Action::SelectPageDown,
            KeyCode::BackTab => Action::IndentLeft,
            KeyCode::Char(c) => Action::InsertChar(c),
            _ => Action::Noop,
        };
    }

    match event.code {
        KeyCode::Up => Action::CursorUp,
        KeyCode::Down => Action::CursorDown,
        KeyCode::Left => Action::CursorLeft,
        KeyCode::Right => Action::CursorRight,
        KeyCode::Home => Action::CursorHome,
        KeyCode::End => Action::CursorEnd,
        KeyCode::PageUp => Action::CursorPageUp,
        KeyCode::PageDown => Action::CursorPageDown,
        KeyCode::Enter => Action::Enter,
        KeyCode::Tab => Action::IndentRight,
        KeyCode::Backspace => Action::Backspace,
        KeyCode::Delete => Action::DeleteChar,
        KeyCode::Esc => Action::Quit,
        KeyCode::Char(c) => Action::InsertChar(c),
        _ => Action::Noop,
    }
}

fn map_search(event: KeyEvent) -> Action {
    match event.code {
        KeyCode::Esc => Action::SearchCancel,
        KeyCode::Enter => Action::SearchConfirm,
        KeyCode::Backspace => Action::SearchBackspace,
        KeyCode::Char(c) => {
            if event.modifiers.contains(KeyModifiers::CONTROL) {
                match c {
                    'g' => Action::SearchNext,
                    _ => Action::Noop,
                }
            } else {
                Action::SearchInsertChar(c)
            }
        }
        KeyCode::Down | KeyCode::Tab => Action::SearchNext,
        KeyCode::Up | KeyCode::BackTab => Action::SearchPrev,
        _ => Action::Noop,
    }
}
