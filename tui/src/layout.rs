use crate::memory_state::{MemoryWorldState, MemoryHealth};
use crate::status::SessionStatus;
use crate::theme::Theme;
use crate::widgets::Widget;
use crate::widgets::activity_feed::{ActivityFeed, FeedEntry};
use crate::widgets::memory_panel;
use crate::ambient::matrix_rain::MatrixRain;
use crate::scanner::DetectedSession;
use crate::mascot::root_v2::{self, RootState, RootTheme, ThemeVariant};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Direction, Rect};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use std::sync::{Arc, Mutex};

pub fn render(
    frame: &mut Frame,
    paired_sessions: &[(DetectedSession, Option<SessionStatus>)],
    primary_status: &SessionStatus,
    theme: &Theme,
    tick: u64,
    feed_entries: &Arc<Mutex<Vec<FeedEntry>>>,
    _rain: &MatrixRain,
    vis_name: &str,
    rain_visible: bool,
    vis_visible: bool,
    memory_world_state: &Option<MemoryWorldState>,
    memory_health: &Option<MemoryHealth>,
    memory_visible: bool,
) -> (Option<Rect>, Rect) {
    let area = frame.area();

    // Sessions panel height: 5 lines per session (mascot height) + separator + border
    let session_count = paired_sessions.len().max(1);
    let content_lines = session_count as u16 * 5 + (session_count.saturating_sub(1)) as u16;
    let sessions_height = (content_lines + 2).min(area.height * 2 / 3);

    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),              // header
            Constraint::Length(sessions_height), // sessions
            Constraint::Min(0),                // bottom (activity feed)
            Constraint::Length(1),              // footer
        ])
        .split(area);

    // ── Header ──
    render_header(frame, main[0], primary_status, paired_sessions, theme, tick, memory_world_state);

    // ── Sessions ──
    render_sessions(frame, main[1], paired_sessions, theme, tick);

    // ── Bottom: Activity feed + optional Memory panel + optional Visualizer ──
    let bottom = main[2];
    let (feed_area, mem_area, vis_area) = match (memory_visible, vis_visible) {
        (true, true) => {
            // 3-way split: 40% feed / 30% memory / 30% vis
            let bottom_split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Percentage(30),
                    Constraint::Percentage(30),
                ])
                .split(bottom);

            let vis_title = format!(" \u{266B} {} [v cycle \u{2502} V hide] ", vis_name);
            let vis_block = Block::default()
                .title(vis_title)
                .borders(Borders::ALL)
                .border_set(theme.border_set())
                .border_style(Style::default().fg(theme.dimmed));
            let vis_inner = vis_block.inner(bottom_split[2]);
            frame.render_widget(vis_block, bottom_split[2]);

            (bottom_split[0], Some(bottom_split[1]), Some(vis_inner))
        }
        (true, false) => {
            // 2-way split: 55% feed / 45% memory
            let bottom_split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(55),
                    Constraint::Percentage(45),
                ])
                .split(bottom);

            (bottom_split[0], Some(bottom_split[1]), None)
        }
        (false, true) => {
            // 2-way split: 55% feed / 45% vis
            let bottom_split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(55),
                    Constraint::Percentage(45),
                ])
                .split(bottom);

            let vis_title = format!(" \u{266B} {} [v cycle \u{2502} V hide] ", vis_name);
            let vis_block = Block::default()
                .title(vis_title)
                .borders(Borders::ALL)
                .border_set(theme.border_set())
                .border_style(Style::default().fg(theme.dimmed));
            let vis_inner = vis_block.inner(bottom_split[1]);
            frame.render_widget(vis_block, bottom_split[1]);

            (bottom_split[0], None, Some(vis_inner))
        }
        (false, false) => {
            // Feed takes full bottom area
            (bottom, None, None)
        }
    };

    let feed = ActivityFeed::new(feed_entries.clone());
    feed.render(frame, feed_area, primary_status, theme, tick);

    if let Some(mem_rect) = mem_area {
        memory_panel::render_memory_panel(frame, mem_rect, memory_world_state, memory_health, theme, tick);
    }

    // ── Footer: shortcut hints ──
    render_footer(frame, main[3], theme, rain_visible, vis_visible, memory_visible);

    (vis_area, bottom)
}

fn render_footer(frame: &mut Frame, area: Rect, theme: &Theme, rain_visible: bool, vis_visible: bool, memory_visible: bool) {
    let rain_label = if rain_visible { "rain off" } else { "rain on" };
    let vis_label = if vis_visible { "vis off" } else { "vis on" };
    let mem_label = if memory_visible { " mem off" } else { " memory" };
    let hints = Line::from(vec![
        Span::styled(" q", Style::default().fg(theme.primary)),
        Span::styled(" quit", Style::default().fg(theme.dimmed)),
        Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)),
        Span::styled("T", Style::default().fg(theme.primary)),
        Span::styled(" theme", Style::default().fg(theme.dimmed)),
        Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)),
        Span::styled("A", Style::default().fg(theme.primary)),
        Span::styled(" activity", Style::default().fg(theme.dimmed)),
        Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)),
        Span::styled("H", Style::default().fg(theme.primary)),
        Span::styled(" history", Style::default().fg(theme.dimmed)),
        Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)),
        Span::styled("S", Style::default().fg(theme.primary)),
        Span::styled(" save", Style::default().fg(theme.dimmed)),
        Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)),
        Span::styled("R", Style::default().fg(theme.primary)),
        Span::styled(format!(" {}", rain_label), Style::default().fg(theme.dimmed)),
        Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)),
        Span::styled("v", Style::default().fg(theme.primary)),
        Span::styled(" cycle", Style::default().fg(theme.dimmed)),
        Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)),
        Span::styled("V", Style::default().fg(theme.primary)),
        Span::styled(format!(" {}", vis_label), Style::default().fg(theme.dimmed)),
        Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)),
        Span::styled("F", Style::default().fg(theme.primary)),
        Span::styled(" flies", Style::default().fg(theme.dimmed)),
        Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)),
        Span::styled("L", Style::default().fg(theme.primary)),
        Span::styled(" lava", Style::default().fg(theme.dimmed)),
        Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)),
        Span::styled("M", Style::default().fg(theme.primary)),
        Span::styled(mem_label, Style::default().fg(theme.dimmed)),
        Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)),
        Span::styled("W", Style::default().fg(theme.primary)),
        Span::styled(" world", Style::default().fg(theme.dimmed)),
    ]);
    frame.render_widget(Paragraph::new(hints), area);
}

fn render_header(
    frame: &mut Frame,
    area: Rect,
    status: &SessionStatus,
    paired_sessions: &[(DetectedSession, Option<SessionStatus>)],
    theme: &Theme,
    tick: u64,
    memory_world_state: &Option<MemoryWorldState>,
) {
    let spinner_chars = ['\u{280B}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283C}', '\u{2834}', '\u{2826}', '\u{2827}', '\u{2807}', '\u{280F}'];
    let spinner = spinner_chars[(tick as usize) % spinner_chars.len()];
    let n = paired_sessions.len();

    let state_color = match status.helix_state() {
        crate::status::HelixState::Thinking => theme.thinking,
        crate::status::HelixState::Coding => theme.success,
        crate::status::HelixState::Streaming => theme.secondary,
        crate::status::HelixState::Error => theme.error,
        crate::status::HelixState::Critical => theme.error,
        crate::status::HelixState::Deep => theme.warning,
        crate::status::HelixState::Done => theme.success,
        _ => theme.primary,
    };

    // Total tokens across all sessions
    let total_tokens: u64 = paired_sessions.iter()
        .filter_map(|(_, s)| s.as_ref())
        .map(|s| s.tokens.input + s.tokens.output)
        .sum();

    // Live clock
    let clock = chrono::Local::now().format("%H:%M:%S").to_string();

    let left = format!(
        " {} HELIX \u{2502} {} session{}",
        spinner, n, if n != 1 { "s" } else { "" }
    );

    let right_tokens = format!("{} tok", fmt_tokens(total_tokens));
    let right_clock = format!(" \u{2502} {}", clock);

    // Build spans
    let mut spans = vec![
        Span::styled(left, Style::default().fg(state_color)),
    ];

    // Memory indicators (if available)
    if let Some(ws) = memory_world_state {
        spans.push(Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)));
        spans.push(Span::styled(format!("{}", ws.entity_count), Style::default().fg(theme.secondary)));
        spans.push(Span::styled(" ent", Style::default().fg(theme.dimmed)));
        if ws.pending_action_count > 0 {
            spans.push(Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)));
            spans.push(Span::styled(format!("{}", ws.pending_action_count), Style::default().fg(theme.warning)));
            spans.push(Span::styled(" act", Style::default().fg(theme.dimmed)));
        }
        let dot_color = if ws.is_stale() { theme.warning } else { theme.success };
        spans.push(Span::styled(" \u{25CF}", Style::default().fg(dot_color)));
    }

    // Right-align: fill space then show stats
    let left_width: usize = spans.iter().map(|s| s.content.len()).sum();
    let right_total = right_tokens.len() + right_clock.len();
    let padding = (area.width as usize).saturating_sub(left_width + right_total);
    if padding > 0 {
        spans.push(Span::raw(" ".repeat(padding)));
    }
    spans.push(Span::styled(right_tokens, Style::default().fg(theme.dimmed)));
    spans.push(Span::styled(right_clock, Style::default().fg(Color::Rgb(255, 255, 255))));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_sessions(
    frame: &mut Frame,
    area: Rect,
    paired_sessions: &[(DetectedSession, Option<SessionStatus>)],
    theme: &Theme,
    tick: u64,
) {
    let block = Block::default()
        .title(" Sessions ")
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.dimmed));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if paired_sessions.is_empty() {
        let lines = vec![
            Line::from(Span::styled("  No AI CLIs detected", Style::default().fg(theme.dimmed))),
            Line::from(Span::styled("  Start Claude Code, Codex, or Gemini CLI", Style::default().fg(theme.dimmed))),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    for (i, (session, status_opt)) in paired_sessions.iter().enumerate() {
        if lines.len() as u16 + 5 > inner.height {
            break;
        }

        let cli_color = cli_to_color(&session.cli, &session.cwd, theme);
        let has_status = status_opt.is_some();

        // Use per-session status if available, otherwise create a minimal default
        let default_status = SessionStatus::default();
        let status = status_opt.as_ref().unwrap_or(&default_status);

        // If status hasn't been updated in >30s, treat as idle
        let now = chrono::Utc::now().timestamp() as u64;
        let is_stale = has_status && status.timestamp > 0 && now.saturating_sub(status.timestamp) > 5;

        // Get v2 mascot lines for this session
        let root_state = if is_stale {
            RootState::Idle
        } else if has_status {
            helix_to_root(status.helix_state())
        } else {
            RootState::Idle
        };
        let root_theme = make_root_theme(theme, &session.cli, &session.cwd);
        // Offset tick per session so mascots don't blink in sync
        let session_tick = tick.wrapping_add(i as u64 * 17);
        let mascot_lines = root_v2::root_lines(root_theme, root_state, session_tick);

        // Build info lines (right side of mascot)
        let dot = if tick % 20 < 15 { "\u{25CF}" } else { "\u{25CB}" };
        let cli_icon = cli_to_icon(&session.cli);

        // Line 0: CLI name + project + human-readable state
        let state_str = if is_stale {
            "Waiting for input".to_string()
        } else if has_status && !status.state.is_empty() {
            match status.state.as_str() {
                "streaming" => "\u{2195} Generating...".to_string(),
                "receiving" => "\u{2193} Receiving response...".to_string(),
                "sending" => "\u{2191} Sending request...".to_string(),
                "coding" | "editing" => "Writing code".to_string(),
                "reviewing" | "reading" => "Reading codebase".to_string(),
                "thinking" => "Planning next step".to_string(),
                "committing" => "Committing changes".to_string(),
                "error" => "Error".to_string(),
                "done" | "finished" => "Done".to_string(),
                "idle" => "Waiting for input".to_string(),
                other => other.to_string(),
            }
        } else {
            "running".to_string()
        };

        // Line 1: model
        let model_str = if has_status && !status.model.is_empty() {
            status.model.clone()
        } else {
            "\u{2014}".to_string()
        };

        // Line 2: context bar
        let pct = if has_status { status.used_pct() } else { 0 };

        let ctx_str = if has_status && status.tokens.context_size > 0 {
            let total = status.tokens.input + status.tokens.output;
            let bar = mini_bar(pct, 12);
            format!("{} {}% \u{2502} {} tok", bar, pct, fmt_tokens(total))
        } else if has_status {
            "no token data yet".to_string()
        } else {
            String::new()
        };

        let bar_color = if pct >= 90 { theme.error } else if pct >= 70 { theme.warning } else if pct >= 50 { theme.primary } else { theme.success };

        // Line 3: uptime + git branch + cwd
        let time_str = if has_status && status.session.duration_ms > 0 {
            format!("\u{23F1} {}", fmt_duration(status.session.duration_ms))
        } else {
            String::new()
        };

        let branch_str = if !status.git.branch.is_empty() {
            format!("\u{2387} {}", status.git.branch)
        } else {
            String::new()
        };

        // Get cwd from status file or scanner
        let raw_cwd = if has_status && !status.cwd.is_empty() {
            status.cwd.clone()
        } else if !session.cwd.is_empty() {
            session.cwd.clone()
        } else {
            String::new()
        };

        let (project_name, cwd) = if !raw_cwd.is_empty() {
            let c = raw_cwd.replace('\\', "/");
            let name = c.rsplit('/').next().unwrap_or(&c).to_string();
            let display = if c.len() > 30 { format!("...{}", &c[c.len()-27..]) } else { c };
            (name, display)
        } else {
            ("\u{2014}".to_string(), "\u{2014}".to_string())
        };

        let mem_mb = session.memory / 1_048_576;

        // Build 5 lines: v2 mascot on left, session info on right
        let info_lines: Vec<Vec<Span>> = vec![
            // Line 0: CLI name + project + state
            {
                let mut s = vec![
                    Span::styled(format!("  {} ", cli_icon), Style::default().fg(cli_color)),
                    Span::styled(session.cli.clone(), Style::default().fg(cli_color)),
                ];
                if project_name != "\u{2014}" {
                    s.push(Span::styled(format!(" \u{2502} {}", project_name), Style::default().fg(theme.text)));
                }
                s.push(Span::styled(format!(" {} ", dot), Style::default().fg(theme.success)));
                s.push(Span::styled(state_str.clone(), Style::default().fg(state_to_color(&state_str, theme))));
                s
            },
            // Line 1: Model
            if model_str != "\u{2014}" || has_status {
                vec![Span::styled(format!("  {}", model_str), Style::default().fg(theme.text))]
            } else { vec![] },
            // Line 2: Context/tokens
            if !ctx_str.is_empty() {
                vec![Span::styled(format!("  {}", ctx_str), Style::default().fg(bar_color))]
            } else { vec![] },
            // Line 3: Uptime + git branch + cwd
            {
                let mut p = vec![Span::styled("  ", Style::default())];
                if !time_str.is_empty() {
                    p.push(Span::styled(time_str.clone(), Style::default().fg(theme.dimmed)));
                    p.push(Span::styled("  ", Style::default()));
                }
                if !branch_str.is_empty() {
                    p.push(Span::styled(branch_str.clone(), Style::default().fg(theme.secondary)));
                    p.push(Span::styled("  ", Style::default()));
                }
                p.push(Span::styled(cwd.clone(), Style::default().fg(theme.dimmed)));
                p
            },
            // Line 4: Memory
            vec![Span::styled(format!("  {}MB", mem_mb), Style::default().fg(theme.dimmed))],
        ];

        for (line_idx, mascot_line) in mascot_lines.iter().enumerate() {
            let mut combined = mascot_line.spans.clone();
            if line_idx < info_lines.len() {
                combined.extend(info_lines[line_idx].clone());
            }
            lines.push(Line::from(combined));
        }

        // Separator between sessions — flash on high context
        if i < paired_sessions.len() - 1 {
            let sep_color = if pct >= 90 {
                // Pulse between error and dimmed
                if (tick / 8) % 2 == 0 { theme.error } else { theme.dimmed }
            } else if pct >= 80 {
                theme.warning
            } else {
                theme.dimmed
            };
            lines.push(Line::from(Span::styled(
                " \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
                Style::default().fg(sep_color),
            )));
        }
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

/// Per-session color palettes — hashed from cwd so each session gets a unique color
const SESSION_PALETTES: &[(ratatui::style::Color, ratatui::style::Color)] = &[
    // (optics/accent, frame/cli name)  — bright, dark pairs
    (Color::Rgb(120, 255, 255), Color::Rgb(60, 120, 120)),  // Cyan
    (Color::Rgb(255, 208, 128), Color::Rgb(122, 101, 32)),   // Amber
    (Color::Rgb(255, 144, 176), Color::Rgb(110, 48, 80)),    // Rose
    (Color::Rgb(144, 255, 120), Color::Rgb(58, 110, 48)),    // Lime
    (Color::Rgb(176, 144, 255), Color::Rgb(80, 48, 120)),    // Violet
    (Color::Rgb(160, 208, 255), Color::Rgb(48, 88, 120)),    // Ice
    (Color::Rgb(255, 144, 112), Color::Rgb(120, 48, 32)),    // Ember
    (Color::Rgb(160, 200, 144), Color::Rgb(74, 96, 64)),     // Moss
];

fn session_color(cwd: &str) -> (ratatui::style::Color, ratatui::style::Color) {
    let hash = crate::status::fnv1a_32(crate::status::normalize_cwd(cwd).as_bytes());
    SESSION_PALETTES[(hash as usize) % SESSION_PALETTES.len()]
}

fn cli_to_color(cli: &str, cwd: &str, theme: &Theme) -> ratatui::style::Color {
    if !cwd.is_empty() && cli == "claude-code" {
        return session_color(cwd).0;
    }
    match cli {
        "claude-code" => theme.thinking,
        "codex" => theme.success,
        "gemini" => theme.warning,
        "aider" => theme.secondary,
        _ => theme.primary,
    }
}

fn cli_to_icon(cli: &str) -> &'static str {
    match cli {
        "claude-code" => "\u{25C9}",
        "codex" => "\u{25C8}",
        "gemini" => "\u{2726}",
        "aider" => "\u{25CB}",
        _ => "\u{25CF}",
    }
}

fn state_to_color(state: &str, theme: &Theme) -> ratatui::style::Color {
    match state {
        "thinking" => theme.thinking,
        "coding" | "editing" => theme.success,
        "reviewing" | "reading" => theme.secondary,
        "streaming" => theme.secondary,
        "error" => theme.error,
        "done" | "finished" => theme.success,
        "idle" => theme.dimmed,
        _ => theme.primary,
    }
}

fn mini_bar(pct: u32, width: usize) -> String {
    let filled = ((pct as f64 / 100.0) * width as f64) as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]",
        "\u{2593}".repeat(filled),
        "\u{2591}".repeat(empty),
    )
}

fn fmt_tokens(t: u64) -> String {
    if t >= 1_000_000 { format!("{:.1}M", t as f64 / 1_000_000.0) }
    else if t >= 1_000 { format!("{:.1}K", t as f64 / 1_000.0) }
    else { t.to_string() }
}

fn fmt_duration(ms: u64) -> String {
    let s = ms / 1000;
    let m = s / 60;
    let h = m / 60;
    if h > 0 { format!("{}h{:02}m", h, m % 60) }
    else if m > 0 { format!("{}m{:02}s", m, s % 60) }
    else { format!("{}s", s) }
}

fn helix_to_root(state: crate::status::HelixState) -> RootState {
    match state {
        crate::status::HelixState::Idle => RootState::Idle,
        crate::status::HelixState::Thinking => RootState::Thinking,
        crate::status::HelixState::Coding => RootState::Coding,
        crate::status::HelixState::Reviewing => RootState::Reviewing,
        crate::status::HelixState::Committing => RootState::Committing,
        crate::status::HelixState::Streaming => RootState::Streaming,
        crate::status::HelixState::Done => RootState::Done,
        crate::status::HelixState::Error => RootState::Error,
        crate::status::HelixState::Deep => RootState::Deep,
        crate::status::HelixState::Critical => RootState::Critical,
    }
}

fn feed_dot_color(cwd: &str) -> Color {
    if cwd.is_empty() {
        return Color::Gray;
    }
    let hash = crate::status::fnv1a_32(crate::status::normalize_cwd(cwd).as_bytes());
    const FEED_PALETTES: &[Color] = &[
        Color::Rgb(120, 255, 255),
        Color::Rgb(255, 208, 128),
        Color::Rgb(255, 144, 176),
        Color::Rgb(144, 255, 120),
        Color::Rgb(176, 144, 255),
        Color::Rgb(160, 208, 255),
        Color::Rgb(255, 144, 112),
        Color::Rgb(160, 200, 144),
    ];
    FEED_PALETTES[(hash as usize) % FEED_PALETTES.len()]
}

pub fn render_activity_overlay(
    frame: &mut Frame,
    feed_entries: &Arc<Mutex<Vec<FeedEntry>>>,
    theme: &Theme,
    scroll: u16,
) {
    use crate::widgets::activity_feed::{tool_icon, tool_color, status_indicator, fmt_duration_short};

    let area = frame.area();

    // Semi-transparent background — dark enough to read text, light enough to hint at effects
    frame.render_widget(Clear, area);

    let entries = feed_entries.lock().unwrap();
    let total = entries.len();

    // Compute total lines for scroll clamping
    let mut total_lines: usize = 0;
    for entry in entries.iter() {
        if entry.is_user_message {
            total_lines += 2;
        } else {
            total_lines += 1; // main line
            if !entry.detail.is_empty() || !entry.result.is_empty() { total_lines += 1; }
            total_lines += 1; // blank separator
        }
    }
    let inner_height = area.height.saturating_sub(2) as usize;
    let max_scroll = total_lines.saturating_sub(inner_height) as u16;
    let scroll = scroll.min(max_scroll);

    let title = format!(
        " Activity [{}/{} \u{2191}\u{2193} scroll \u{2502} PgUp/PgDn \u{2502} Esc close] ",
        (scroll as usize).min(total), total
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.primary));

    let mut lines: Vec<Line> = Vec::new();

    for entry in entries.iter() {
        if entry.is_user_message {
            // Thicker user separator
            let sep = format!(" \u{2501}\u{2501}\u{2501} user \u{2501}\u{2501}\u{2501}");
            lines.push(Line::from(Span::styled(sep, Style::default().fg(theme.text))));
            lines.push(Line::from(""));
            continue;
        }

        let icon = tool_icon(&entry.tool);
        let tc = tool_color(&entry.tool, theme);
        let time_str = &entry.time; // Full HH:MM:SS in overlay
        let file_display = if entry.file.is_empty() { String::new() } else { entry.file.clone() };
        let dur_str = fmt_duration_short(entry.duration_ms);
        let (stat_str, stat_color_fn) = status_indicator(&entry.status);
        let stat_color = stat_color_fn.map(|f| f(theme)).unwrap_or(theme.dimmed);

        let desc = if entry.description.is_empty() { entry.tool.clone() } else { entry.description.clone() };

        // Line 1: status + icon + time + description + file + duration
        let mut spans = vec![
            Span::styled(format!(" {}", stat_str), Style::default().fg(stat_color)),
            Span::styled(format!("{} ", icon), Style::default().fg(tc)),
            Span::styled(format!("{}", time_str), Style::default().fg(theme.dimmed)),
            Span::styled("  ", Style::default()),
            Span::styled(desc.clone(), Style::default().fg(tc)),
        ];

        if !file_display.is_empty() {
            spans.push(Span::styled("  ", Style::default()));
            spans.push(Span::styled(file_display.clone(), Style::default().fg(theme.text)));
        }

        if !dur_str.is_empty() {
            spans.push(Span::styled(format!("  {}", dur_str), Style::default().fg(theme.dimmed)));
        }

        lines.push(Line::from(spans));

        // Line 2: detail/result
        let detail_text = if !entry.detail.is_empty() {
            entry.detail.clone()
        } else if !entry.result.is_empty() {
            entry.result.clone()
        } else {
            String::new()
        };

        if !detail_text.is_empty() {
            let inner_w = area.width.saturating_sub(2) as usize;
            let indent = "               ";
            let max_detail = inner_w.saturating_sub(indent.len());
            let truncated = if detail_text.len() > max_detail && max_detail > 3 {
                format!("{}...", &detail_text[..max_detail - 3])
            } else {
                detail_text
            };
            lines.push(Line::from(Span::styled(
                format!("{}{}", indent, truncated),
                Style::default().fg(theme.dimmed),
            )));
        }

        // Blank separator
        lines.push(Line::from(""));
    }

    let paragraph = if lines.is_empty() {
        Paragraph::new("  No activity recorded yet.")
            .style(Style::default().fg(theme.dimmed))
    } else {
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0))
    };

    frame.render_widget(paragraph.block(block), area);
}

fn make_root_theme(theme: &Theme, cli: &str, cwd: &str) -> RootTheme {
    let variant = match theme.name.as_str() {
        "clean" => ThemeVariant::Clean,
        "retro" => ThemeVariant::Retro,
        _ => ThemeVariant::Cyberpunk,
    };

    let (bright, dark) = if !cwd.is_empty() && cli == "claude-code" {
        session_color(cwd)
    } else {
        let c = match cli {
            "claude-code" => theme.thinking,
            "codex" => theme.success,
            "gemini" => theme.warning,
            "aider" => theme.secondary,
            _ => theme.primary,
        };
        (c, theme.dimmed)
    };

    RootTheme {
        variant,
        frame: dark,
        cue: bright,
        optics: bright,
        core: dark,
        alert: theme.error,
    }
}
