/// Renders a Buffer into a ratatui Frame area, with markdown highlighting in preview mode.
use ratatui::prelude::*;
use ratatui::widgets::*;
use crate::editor::{Buffer, Pos};
use crate::markdown::{self, LineKind, SpanKind};

pub struct PaneState {
    pub scroll: usize,
    pub editing: bool,
}

impl PaneState {
    pub fn new() -> Self {
        Self { scroll: 0, editing: true }
    }

    pub fn ensure_cursor_visible(&mut self, cursor_line: usize, height: usize) {
        if height == 0 { return; }
        if cursor_line < self.scroll {
            self.scroll = cursor_line;
        } else if cursor_line >= self.scroll + height {
            self.scroll = cursor_line - height + 1;
        }
    }
}

pub fn render_pane(
    frame: &mut Frame,
    buf: &Buffer,
    state: &PaneState,
    area: Rect,
    focused: bool,
    title: &str,
    search_query: Option<&str>,
    folded_lines: &std::collections::HashSet<usize>,
) {
    let inner_height = area.height.saturating_sub(2) as usize;
    let kinds = markdown::classify_lines(&buf.lines);

    // Build visible lines, skipping folded sections
    let mut visible_lines: Vec<usize> = Vec::new(); // maps visual row -> buffer line index
    let mut i = 0;
    while i < buf.lines.len() {
        visible_lines.push(i);
        if folded_lines.contains(&i) {
            // Skip the section
            if let Some((_, end)) = markdown::heading_section_range(&buf.lines, i) {
                i = end;
                continue;
            }
        }
        i += 1;
    }

    // Find the visual row of the cursor
    let cursor_visual = visible_lines.iter().position(|&l| l == buf.cursor.line).unwrap_or(0);

    // Apply scrolling based on visual position
    let scroll = if cursor_visual < state.scroll {
        cursor_visual
    } else if cursor_visual >= state.scroll + inner_height {
        cursor_visual.saturating_sub(inner_height - 1)
    } else {
        state.scroll
    };

    let visible_range = &visible_lines[scroll..visible_lines.len().min(scroll + inner_height)];

    let mut text_lines: Vec<Line> = Vec::new();
    let search_lower = search_query.map(|q| q.to_lowercase());

    for &line_idx in visible_range {
        let line = &buf.lines[line_idx];
        let kind = &kinds[line_idx];
        let is_cursor_line = focused && line_idx == buf.cursor.line;

        if state.editing {
            // Raw edit mode: show text with cursor and selection
            let chars: Vec<char> = line.chars().collect();
            let char_count = chars.len();
            let mut spans = Vec::new();

            let sel = if focused { buf.selection.as_ref() } else { None };

            for (ci, ch) in chars.iter().enumerate() {
                let in_sel = sel.map_or(false, |s| {
                    let (start, end) = s.ordered();
                    let p = Pos::new(line_idx, ci);
                    p >= start && p < end
                });
                let is_cur = is_cursor_line && ci == buf.cursor.col;

                // Search highlight
                let in_search = search_lower.as_ref().map_or(false, |q| {
                    if q.is_empty() { return false; }
                    let line_lower: String = chars.iter().collect::<String>().to_lowercase();
                    let q_len = q.chars().count();
                    // Check if ci falls within any match
                    let mut pos = 0;
                    let ll_chars: Vec<char> = line_lower.chars().collect();
                    let q_chars: Vec<char> = q.chars().collect();
                    loop {
                        if pos + q_len > ll_chars.len() { break; }
                        if ll_chars[pos..pos+q_len] == q_chars[..] {
                            if ci >= pos && ci < pos + q_len {
                                return true;
                            }
                            pos += 1;
                        } else {
                            pos += 1;
                        }
                    }
                    false
                });

                let style = if is_cur {
                    Style::default().bg(Color::White).fg(Color::Black)
                } else if in_sel {
                    Style::default().bg(Color::LightBlue).fg(Color::Black)
                } else if in_search {
                    Style::default().bg(Color::Yellow).fg(Color::Black)
                } else {
                    Style::default()
                };
                spans.push(Span::styled(ch.to_string(), style));
            }
            if is_cursor_line && buf.cursor.col >= char_count {
                spans.push(Span::styled(" ", Style::default().bg(Color::White).fg(Color::Black)));
            }
            text_lines.push(Line::from(spans));
        } else {
            // Preview mode: render with markdown formatting
            let line_spans = markdown::parse_inline_spans(line, kind);
            let is_folded = folded_lines.contains(&line_idx);

            let mut spans: Vec<Span> = Vec::new();
            let chars: Vec<char> = line.chars().collect();

            for ispan in &line_spans {
                let text: String = chars[ispan.start..ispan.end].iter().collect();
                let style = match (&ispan.kind, kind) {
                    (SpanKind::HeadingMarker, LineKind::Heading(1)) => {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    }
                    (SpanKind::HeadingMarker, LineKind::Heading(2)) => {
                        Style::default().fg(Color::Cyan)
                    }
                    (SpanKind::HeadingMarker, _) => {
                        Style::default().fg(Color::DarkGray)
                    }
                    (SpanKind::Normal, LineKind::Heading(1)) => {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    }
                    (SpanKind::Normal, LineKind::Heading(2)) => {
                        Style::default().fg(Color::Cyan)
                    }
                    (SpanKind::Normal, LineKind::Heading(_)) => {
                        Style::default().fg(Color::Blue)
                    }
                    (SpanKind::ListBullet, _) => {
                        Style::default().fg(Color::DarkGray)
                    }
                    (SpanKind::BlockquoteMarker, _) => {
                        Style::default().fg(Color::DarkGray)
                    }
                    (SpanKind::Normal, LineKind::Blockquote) => {
                        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)
                    }
                    (SpanKind::Bold, _) => {
                        Style::default().add_modifier(Modifier::BOLD)
                    }
                    (SpanKind::Italic, _) => {
                        Style::default().add_modifier(Modifier::ITALIC)
                    }
                    (SpanKind::Code, _) => {
                        Style::default().fg(Color::Green)
                    }
                    _ => Style::default(),
                };
                spans.push(Span::styled(text, style));
            }

            if is_folded {
                spans.push(Span::styled(" ...", Style::default().fg(Color::DarkGray)));
            }

            // Highlight cursor line in preview
            if is_cursor_line {
                if spans.is_empty() {
                    spans.push(Span::styled(" ", Style::default().bg(Color::DarkGray)));
                } else {
                    // Add background highlight to all spans on cursor line
                    for span in &mut spans {
                        span.style = span.style.bg(Color::DarkGray);
                    }
                }
            }

            text_lines.push(Line::from(spans));
        }
    }

    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let mode_indicator = if state.editing { " [EDIT]" } else { "" };
    let dirty_indicator = if buf.dirty { " [+]" } else { "" };
    let full_title = format!(" {}{}{} ", title, mode_indicator, dirty_indicator);

    let paragraph = Paragraph::new(text_lines)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(full_title, border_style.add_modifier(Modifier::BOLD))));

    frame.render_widget(paragraph, area);
}
