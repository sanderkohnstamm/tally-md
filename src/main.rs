mod app;
mod clipboard;
mod editor;
mod finished;
mod keys;
mod markdown;
mod pane;

use std::io;
use std::path::PathBuf;
use crossterm::event::{self, Event, EnableMouseCapture, DisableMouseCapture};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::prelude::*;
use ratatui::widgets::*;

fn todos_dir() -> PathBuf {
    let home = dirs::home_dir().expect("Could not find home directory");
    home.join(".todos")
}

fn main() -> io::Result<()> {
    let dir = todos_dir();
    std::fs::create_dir_all(&dir)?;

    let todo_path = dir.join("todo.md");
    let finished_path = dir.join("finished.md");

    let todo_content = std::fs::read_to_string(&todo_path).unwrap_or_default();
    let finished_content = std::fs::read_to_string(&finished_path).unwrap_or_default();

    // Migration: if todo.md has inline date logs, split them out
    let (todo_content, extra_finished) = if finished_content.is_empty() {
        finished::migrate_inline_log(&todo_content)
    } else {
        (todo_content, String::new())
    };

    let finished_content = if !extra_finished.is_empty() {
        if finished_content.is_empty() {
            extra_finished
        } else {
            format!("{}\n{}", extra_finished, finished_content)
        }
    } else {
        finished_content
    };

    let todo_buf = editor::Buffer::from_string(&todo_content);
    let mut finished_buf = editor::Buffer::from_string(&finished_content);

    // Fill empty days
    let today = chrono::Local::now().date_naive();
    finished::fill_empty_days(&mut finished_buf, today);

    let mut app = app::App::new(todo_buf, finished_buf, todo_path, finished_path);

    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Event loop
    loop {
        terminal.draw(|frame| render(frame, &mut app))?;

        match event::read()? {
            Event::Key(key_event) => {
                let action = keys::map_key(key_event, &app.input_mode);
                let page_size = terminal.size().map(|s| s.height.saturating_sub(4) as usize).unwrap_or(20);
                app.update(action, page_size);
            }
            Event::Mouse(mouse_event) => {
                let action = keys::map_mouse(mouse_event);
                app.update(action, 20);
            }
            Event::Resize(_, _) => {
                // Terminal handles redraw
            }
            _ => {}
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;

    Ok(())
}

fn render(frame: &mut Frame, app: &mut app::App) {
    let area = frame.area();

    // Layout: two panes + status bar
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let pane_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(main_chunks[0]);

    let inner_height = pane_chunks[0].height.saturating_sub(2) as usize;

    // Ensure cursor visible
    app.todo_pane.ensure_cursor_visible(app.todo_buf.cursor.line, inner_height);
    app.finished_pane.ensure_cursor_visible(app.finished_buf.cursor.line, inner_height);

    let search_q = if app.input_mode == keys::InputMode::Search {
        Some(app.search_query.as_str())
    } else {
        None
    };

    // Render panes
    pane::render_pane(
        frame,
        &app.todo_buf,
        &app.todo_pane,
        pane_chunks[0],
        app.focus == app::FocusPane::Todo,
        "todo.md",
        if app.focus == app::FocusPane::Todo { search_q } else { None },
        &app.folded,
    );

    pane::render_pane(
        frame,
        &app.finished_buf,
        &app.finished_pane,
        pane_chunks[1],
        app.focus == app::FocusPane::Finished,
        "finished.md",
        if app.focus == app::FocusPane::Finished { search_q } else { None },
        &std::collections::HashSet::new(),
    );

    // Status bar
    render_status_bar(frame, app, main_chunks[1]);
}

fn render_status_bar(frame: &mut Frame, app: &app::App, area: Rect) {
    let msg = app.message.as_deref().unwrap_or("");

    let status = if app.input_mode == keys::InputMode::Search {
        format!(" Search: {}█  (Enter/↑↓:navigate  Esc:cancel)", app.search_query)
    } else {
        let mode = if app.active_pane().editing { "EDIT" } else { "PREVIEW" };
        let help = "^S:save  ^E:edit/preview  ^Enter:complete  ^F:search  ^Tab:switch  ^Z:undo  Esc:quit";
        if !msg.is_empty() {
            format!(" [{}] {}  │  {}", mode, msg, help)
        } else {
            format!(" [{}] {}", mode, help)
        }
    };

    let bar = Paragraph::new(status)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    frame.render_widget(bar, area);
}
