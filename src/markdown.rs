/// Markdown line classification and inline span detection for rendering.

#[derive(Debug, Clone, PartialEq)]
pub enum LineKind {
    Heading(u8),       // # level 1-6
    ListItem(usize),   // indentation in spaces
    Blockquote,        // > text
    CodeFence,         // ```
    CodeContent,       // inside fenced code block
    BlankLine,
    Paragraph,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpanKind {
    Normal,
    Bold,
    Italic,
    Code,
    HeadingMarker,
    ListBullet,
    BlockquoteMarker,
}

#[derive(Debug, Clone)]
pub struct InlineSpan {
    pub start: usize, // char offset
    pub end: usize,   // char offset (exclusive)
    pub kind: SpanKind,
}

pub fn classify_line(line: &str, in_code_block: bool) -> LineKind {
    if in_code_block {
        if line.trim_start().starts_with("```") {
            return LineKind::CodeFence;
        }
        return LineKind::CodeContent;
    }

    let trimmed = line.trim();
    if trimmed.is_empty() {
        return LineKind::BlankLine;
    }
    if trimmed.starts_with("```") {
        return LineKind::CodeFence;
    }
    if trimmed.starts_with("# ") || trimmed == "#" {
        return LineKind::Heading(1);
    }
    if trimmed.starts_with("## ") || trimmed == "##" {
        return LineKind::Heading(2);
    }
    if trimmed.starts_with("### ") || trimmed == "###" {
        return LineKind::Heading(3);
    }
    if trimmed.starts_with("#### ") {
        return LineKind::Heading(4);
    }
    if trimmed.starts_with("##### ") {
        return LineKind::Heading(5);
    }
    if trimmed.starts_with("###### ") {
        return LineKind::Heading(6);
    }
    let stripped = line.trim_start();
    if stripped.starts_with("- ") || stripped == "-" {
        let indent = line.len() - stripped.len();
        return LineKind::ListItem(indent);
    }
    if stripped.starts_with("> ") || stripped == ">" {
        return LineKind::Blockquote;
    }
    LineKind::Paragraph
}

/// Classify all lines, tracking code fence state.
pub fn classify_lines(lines: &[String]) -> Vec<LineKind> {
    let mut result = Vec::new();
    let mut in_code = false;
    for line in lines {
        let kind = classify_line(line, in_code);
        if kind == LineKind::CodeFence {
            in_code = !in_code;
        }
        result.push(kind);
    }
    result
}

/// Parse inline formatting spans for a single line.
pub fn parse_inline_spans(line: &str, kind: &LineKind) -> Vec<InlineSpan> {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();

    match kind {
        LineKind::Heading(level) => {
            let marker_len = *level as usize + 1; // "# " = 2, "## " = 3, etc.
            let marker_end = marker_len.min(len);
            let mut spans = vec![
                InlineSpan { start: 0, end: marker_end, kind: SpanKind::HeadingMarker },
            ];
            if marker_end < len {
                spans.push(InlineSpan { start: marker_end, end: len, kind: SpanKind::Normal });
            }
            spans
        }
        LineKind::ListItem(indent) => {
            let bullet_end = (*indent + 2).min(len); // indent + "- "
            let mut spans = vec![
                InlineSpan { start: 0, end: bullet_end, kind: SpanKind::ListBullet },
            ];
            if bullet_end < len {
                spans.extend(parse_inline_formatting(&chars[bullet_end..], bullet_end));
            }
            spans
        }
        LineKind::Blockquote => {
            let marker_end = if len >= 2 { 2 } else { len }; // "> "
            let mut spans = vec![
                InlineSpan { start: 0, end: marker_end, kind: SpanKind::BlockquoteMarker },
            ];
            if marker_end < len {
                spans.extend(parse_inline_formatting(&chars[marker_end..], marker_end));
            }
            spans
        }
        LineKind::CodeFence | LineKind::CodeContent => {
            vec![InlineSpan { start: 0, end: len, kind: SpanKind::Code }]
        }
        LineKind::BlankLine => {
            vec![]
        }
        LineKind::Paragraph => {
            parse_inline_formatting(&chars, 0)
        }
    }
}

fn parse_inline_formatting(chars: &[char], offset: usize) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    let len = chars.len();
    let mut i = 0;
    let mut normal_start = 0;

    while i < len {
        // Inline code: `...`
        if chars[i] == '`' {
            if i > normal_start {
                spans.push(InlineSpan {
                    start: offset + normal_start,
                    end: offset + i,
                    kind: SpanKind::Normal,
                });
            }
            let start = i;
            i += 1;
            while i < len && chars[i] != '`' {
                i += 1;
            }
            if i < len {
                i += 1; // consume closing `
            }
            spans.push(InlineSpan {
                start: offset + start,
                end: offset + i,
                kind: SpanKind::Code,
            });
            normal_start = i;
            continue;
        }
        // Bold: **...**
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            if i > normal_start {
                spans.push(InlineSpan {
                    start: offset + normal_start,
                    end: offset + i,
                    kind: SpanKind::Normal,
                });
            }
            let start = i;
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '*') {
                i += 1;
            }
            if i + 1 < len {
                i += 2; // consume closing **
            }
            spans.push(InlineSpan {
                start: offset + start,
                end: offset + i,
                kind: SpanKind::Bold,
            });
            normal_start = i;
            continue;
        }
        // Italic: *...*
        if chars[i] == '*' && (i + 1 < len && chars[i + 1] != '*') {
            if i > normal_start {
                spans.push(InlineSpan {
                    start: offset + normal_start,
                    end: offset + i,
                    kind: SpanKind::Normal,
                });
            }
            let start = i;
            i += 1;
            while i < len && chars[i] != '*' {
                i += 1;
            }
            if i < len {
                i += 1; // consume closing *
            }
            spans.push(InlineSpan {
                start: offset + start,
                end: offset + i,
                kind: SpanKind::Italic,
            });
            normal_start = i;
            continue;
        }
        i += 1;
    }

    if normal_start < len {
        spans.push(InlineSpan {
            start: offset + normal_start,
            end: offset + len,
            kind: SpanKind::Normal,
        });
    }

    if spans.is_empty() && len > 0 {
        spans.push(InlineSpan {
            start: offset,
            end: offset + len,
            kind: SpanKind::Normal,
        });
    }

    spans
}

/// Compute breadcrumb (parent list items) for a list item at given line index.
pub fn breadcrumb_for(lines: &[String], line_idx: usize) -> Vec<String> {
    let target_line = &lines[line_idx];
    let target_stripped = target_line.trim_start();
    if !target_stripped.starts_with("- ") {
        return Vec::new();
    }
    let target_indent = target_line.len() - target_stripped.len();

    let mut crumbs = Vec::new();
    let mut current_indent = target_indent;

    for i in (0..line_idx).rev() {
        let line = &lines[i];
        let stripped = line.trim_start();
        if !stripped.starts_with("- ") {
            continue;
        }
        let indent = line.len() - stripped.len();
        if indent < current_indent {
            let text = stripped.strip_prefix("- ").unwrap_or(stripped);
            crumbs.push(text.to_string());
            current_indent = indent;
            if indent == 0 {
                break;
            }
        }
    }

    crumbs.reverse();
    crumbs
}

/// Get the heading section range (for folding).
/// Returns (start, end) where end is exclusive.
pub fn heading_section_range(lines: &[String], line_idx: usize) -> Option<(usize, usize)> {
    let kinds = classify_lines(lines);
    let level = match &kinds[line_idx] {
        LineKind::Heading(l) => *l,
        _ => return None,
    };
    let end = (line_idx + 1..lines.len())
        .find(|&i| matches!(&kinds[i], LineKind::Heading(l) if *l <= level))
        .unwrap_or(lines.len());
    Some((line_idx, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify() {
        assert_eq!(classify_line("# Hello", false), LineKind::Heading(1));
        assert_eq!(classify_line("## World", false), LineKind::Heading(2));
        assert_eq!(classify_line("- item", false), LineKind::ListItem(0));
        assert_eq!(classify_line("  - nested", false), LineKind::ListItem(2));
        assert_eq!(classify_line("> quote", false), LineKind::Blockquote);
        assert_eq!(classify_line("```", false), LineKind::CodeFence);
        assert_eq!(classify_line("hello", true), LineKind::CodeContent);
        assert_eq!(classify_line("", false), LineKind::BlankLine);
        assert_eq!(classify_line("plain text", false), LineKind::Paragraph);
    }

    #[test]
    fn test_breadcrumb() {
        let lines: Vec<String> = vec![
            "- Work".to_string(),
            "  - Backend".to_string(),
            "    - Fix bug".to_string(),
        ];
        assert_eq!(breadcrumb_for(&lines, 2), vec!["Work", "Backend"]);
        assert_eq!(breadcrumb_for(&lines, 1), vec!["Work"]);
        assert!(breadcrumb_for(&lines, 0).is_empty());
    }

    #[test]
    fn test_heading_section() {
        let lines: Vec<String> = vec![
            "# A".to_string(),
            "text".to_string(),
            "## B".to_string(),
            "more".to_string(),
            "# C".to_string(),
        ];
        assert_eq!(heading_section_range(&lines, 0), Some((0, 4)));
        assert_eq!(heading_section_range(&lines, 2), Some((2, 4)));
        assert_eq!(heading_section_range(&lines, 4), Some((4, 5)));
    }
}
