use crate::history::{format_duration_short, format_relative_time, HistoryEntry};
use crate::theme::Theme;
use chrono::DateTime;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

/// Render the full-screen session history overlay.
pub fn render_history_overlay(
    frame: &mut Frame,
    entries: &[HistoryEntry],
    selected: usize,
    delete_confirm: bool,
    theme: &Theme,
) {
    let area = frame.area();

    // Clear background
    frame.render_widget(Clear, area);

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.dimmed))
        .style(Style::default().bg(Color::Black));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // 4 vertical sections: header (1), list (55%), detail (40%), footer (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Percentage(55),
            Constraint::Percentage(40),
            Constraint::Length(1),
        ])
        .split(inner);

    render_header(frame, chunks[0], entries.len(), theme);
    render_session_list(frame, chunks[1], entries, selected, theme);
    if !entries.is_empty() && selected < entries.len() {
        render_detail_panel(frame, chunks[2], &entries[selected], theme);
    }
    render_footer(frame, chunks[3], delete_confirm, theme);
}

/// Header: " SESSION HISTORY (N sessions, last 30 days) "
fn render_header(frame: &mut Frame, area: Rect, count: usize, theme: &Theme) {
    let text = format!(" SESSION HISTORY ({} sessions, last 30 days) ", count);
    let header = Paragraph::new(Line::from(Span::styled(
        text,
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(header, area);
}

/// Scrollable session list with selection marker.
fn render_session_list(
    frame: &mut Frame,
    area: Rect,
    entries: &[HistoryEntry],
    selected: usize,
    theme: &Theme,
) {
    let visible_height = area.height as usize;
    if visible_height == 0 || entries.is_empty() {
        return;
    }

    // Auto-scroll to keep selected visible
    let scroll_offset = if selected >= visible_height {
        selected - visible_height + 1
    } else {
        0
    };

    let mut lines: Vec<Line> = Vec::with_capacity(visible_height);

    for i in scroll_offset..(scroll_offset + visible_height).min(entries.len()) {
        let entry = &entries[i];
        let is_selected = i == selected;

        // Marker
        let marker = if is_selected { "▸ " } else { "  " };

        // Relative time (12 chars)
        let time_str = parse_timestamp_relative(&entry.started_at);
        let time_col = format!("{:<12}", truncate_str(&time_str, 12));

        // Project name: last path segment of cwd (30 chars)
        let project = last_path_segment(&entry.cwd);
        let project_col = format!("{:<30}", truncate_str(&project, 30));

        // Git branch (16 chars)
        let branch = if entry.git_branch.is_empty() {
            "—".to_string()
        } else {
            entry.git_branch.clone()
        };
        let branch_col = format!("{:<16}", truncate_str(&branch, 16));

        // Duration (6 chars right-aligned)
        let dur = format_duration_short(entry.duration_ms);
        let dur_col = format!("{:>6}", truncate_str(&dur, 6));

        // Context % (4 chars right-aligned)
        let ctx = format!("{}%", entry.context_used_pct);
        let ctx_col = format!("{:>4}", truncate_str(&ctx, 4));

        // Color for context %
        let ctx_color = if entry.context_used_pct >= 80 {
            Color::Red
        } else if entry.context_used_pct >= 50 {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        let base_style = if is_selected {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        let branch_style = if is_selected {
            base_style
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let ctx_style = if is_selected {
            base_style
        } else {
            Style::default().fg(ctx_color)
        };

        lines.push(Line::from(vec![
            Span::styled(marker.to_string(), base_style),
            Span::styled(time_col, base_style),
            Span::styled(project_col, base_style),
            Span::styled(branch_col, branch_style),
            Span::styled(dur_col, base_style),
            Span::styled(" ", base_style),
            Span::styled(ctx_col, ctx_style),
        ]));
    }

    let list = Paragraph::new(lines);
    frame.render_widget(list, area);
}

/// Detail panel for the selected session.
fn render_detail_panel(frame: &mut Frame, area: Rect, entry: &HistoryEntry, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.dimmed));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    // Line 1: Project · branch · time ago
    let project = last_path_segment(&entry.cwd);
    let time_str = parse_timestamp_relative(&entry.started_at);
    let branch_display = if entry.git_branch.is_empty() {
        String::new()
    } else {
        format!(" · {}", entry.git_branch)
    };
    lines.push(Line::from(vec![
        Span::styled(
            project,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(branch_display, Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" · {}", time_str), Style::default().fg(Color::DarkGray)),
    ]));

    // Line 2: Model
    lines.push(Line::from(Span::styled(
        format!("Model: {}", entry.model),
        Style::default().fg(Color::Gray),
    )));

    // Line 3: Tokens + duration
    let tokens_in = format_tokens(entry.tokens_in);
    let tokens_out = format_tokens(entry.tokens_out);
    let dur = format_duration_short(entry.duration_ms);
    lines.push(Line::from(Span::styled(
        format!(
            "Tokens: {}in / {}out ({}% context) · {}",
            tokens_in, tokens_out, entry.context_used_pct, dur
        ),
        Style::default().fg(Color::Gray),
    )));

    // Line 4: Last activity
    if !entry.last_tool.is_empty() {
        let file_part = if entry.last_file.is_empty() {
            String::new()
        } else {
            format!(" {}", last_path_segment(&entry.last_file))
        };
        let desc_part = if entry.last_description.is_empty() {
            String::new()
        } else {
            format!(" — \"{}\"", truncate_str(&entry.last_description, 60))
        };
        lines.push(Line::from(Span::styled(
            format!("Last: {}{}{}", entry.last_tool, file_part, desc_part),
            Style::default().fg(Color::Gray),
        )));
    }

    // Blank separator
    lines.push(Line::from(""));

    // Files touched (up to 5)
    if !entry.files_touched.is_empty() {
        let show_count = entry.files_touched.len().min(5);
        let mut file_spans: Vec<Span> = vec![Span::styled(
            "Files: ",
            Style::default().fg(Color::DarkGray),
        )];
        for (i, f) in entry.files_touched[..show_count].iter().enumerate() {
            if i > 0 {
                file_spans.push(Span::styled(", ", Style::default().fg(Color::DarkGray)));
            }
            file_spans.push(Span::styled(
                last_path_segment(f),
                Style::default().fg(Color::Gray),
            ));
        }
        if entry.files_touched.len() > 5 {
            file_spans.push(Span::styled(
                format!(" +{} more", entry.files_touched.len() - 5),
                Style::default().fg(Color::DarkGray),
            ));
        }
        lines.push(Line::from(file_spans));
        lines.push(Line::from(""));
    }

    // Activity log (up to 10 entries)
    if entry.activities.is_empty() {
        lines.push(Line::from(Span::styled(
            "No activity recorded (dashboard was not running)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let show_count = entry.activities.len().min(10);
        for act in &entry.activities[..show_count] {
            let status_color = match act.status.as_str() {
                "success" | "ok" => Color::Green,
                "failure" | "error" | "fail" => Color::Red,
                _ => Color::DarkGray,
            };
            let file_part = if act.file.is_empty() {
                String::new()
            } else {
                format!(" {}", last_path_segment(&act.file))
            };
            let desc_part = if act.description.is_empty() {
                String::new()
            } else {
                format!(" — {}", truncate_str(&act.description, 50))
            };
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled("● ", Style::default().fg(status_color)),
                Span::styled(
                    format!("{}{}{}", act.tool, file_part, desc_part),
                    Style::default().fg(Color::Gray),
                ),
            ]));
        }
        if entry.activities.len() > 10 {
            lines.push(Line::from(Span::styled(
                format!("  ... +{} more", entry.activities.len() - 10),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    let detail = Paragraph::new(lines);
    frame.render_widget(detail, inner);
}

/// Footer with keybinding hints.
fn render_footer(frame: &mut Frame, area: Rect, delete_confirm: bool, theme: &Theme) {
    let text = if delete_confirm {
        " Press D again to confirm delete, any other key to cancel "
    } else {
        " [Enter] Resume  [↑↓] Navigate  [PgUp/Dn] Fast  [D] Delete  [Esc] Close "
    };
    let footer = Paragraph::new(Line::from(Span::styled(
        text,
        Style::default().fg(if delete_confirm {
            Color::Red
        } else {
            theme.dimmed
        }),
    )));
    frame.render_widget(footer, area);
}

/// Format token count as human-readable: "1.2M", "45.3k", or raw number.
fn format_tokens(t: u64) -> String {
    if t >= 1_000_000 {
        format!("{:.1}M", t as f64 / 1_000_000.0)
    } else if t >= 1_000 {
        format!("{:.1}k", t as f64 / 1_000.0)
    } else {
        format!("{}", t)
    }
}

/// Extract last path segment from a path string (project name).
fn last_path_segment(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    normalized
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(path)
        .to_string()
}

/// Truncate a string to max_len characters, adding "…" if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else if max_len <= 1 {
        "…".to_string()
    } else {
        let mut result: String = s.chars().take(max_len - 1).collect();
        result.push('…');
        result
    }
}

/// Parse an RFC3339 timestamp string and return a relative time string.
fn parse_timestamp_relative(timestamp_str: &str) -> String {
    if let Ok(dt) = DateTime::parse_from_rfc3339(timestamp_str) {
        let epoch = dt.timestamp() as u64;
        format_relative_time(epoch)
    } else {
        "unknown".to_string()
    }
}
