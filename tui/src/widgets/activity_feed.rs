use crate::status::{self, SessionStatus};
use crate::theme::Theme;
use crate::widgets::Widget;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::text::{Line, Span};
use std::sync::{Arc, Mutex};

/// Same palette as layout.rs — session color by cwd hash
const SESSION_PALETTES: &[Color] = &[
    Color::Rgb(120, 255, 255),  // Cyan
    Color::Rgb(255, 208, 128),  // Amber
    Color::Rgb(255, 144, 176),  // Rose
    Color::Rgb(144, 255, 120),  // Lime
    Color::Rgb(176, 144, 255),  // Violet
    Color::Rgb(160, 208, 255),  // Ice
    Color::Rgb(255, 144, 112),  // Ember
    Color::Rgb(160, 200, 144),  // Moss
];

/// Linearly interpolate between two colors
fn lerp_color(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (r1, g1, b1) = color_to_rgb(from);
    let (r2, g2, b2) = color_to_rgb(to);
    Color::Rgb(
        (r1 as f32 + (r2 as f32 - r1 as f32) * t) as u8,
        (g1 as f32 + (g2 as f32 - g1 as f32) * t) as u8,
        (b1 as f32 + (b2 as f32 - b1 as f32) * t) as u8,
    )
}

fn color_to_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Gray => (128, 128, 128),
        Color::DarkGray => (80, 80, 80),
        Color::White => (255, 255, 255),
        Color::Green => (0, 255, 0),
        Color::Yellow => (255, 255, 0),
        Color::Red => (255, 0, 0),
        Color::Blue => (0, 0, 255),
        Color::Cyan => (0, 255, 255),
        Color::Magenta => (255, 0, 255),
        _ => (128, 128, 128),
    }
}

fn session_dot_color(cwd: &str) -> Color {
    if cwd.is_empty() {
        return Color::Gray;
    }
    let hash = status::fnv1a_32(status::normalize_cwd(cwd).as_bytes());
    SESSION_PALETTES[(hash as usize) % SESSION_PALETTES.len()]
}

// ── EntryStatus ──

#[derive(Clone, Debug)]
pub enum EntryStatus {
    Pending,
    Success,
    Failure,
    Warning,
    Neutral,
}

// ── FeedEntry ──

#[derive(Clone)]
pub struct FeedEntry {
    pub time: String,
    pub timestamp_ms: u64,
    pub tool: String,
    pub file: String,
    pub cwd: String,
    pub description: String,
    pub detail: String,
    pub result: String,
    pub duration_ms: u64,
    pub status: EntryStatus,
    pub is_user_message: bool,
    pub group_key: String,
}

// ── Public helpers (called from layout.rs) ──

pub fn tool_icon(tool: &str) -> &'static str {
    match tool {
        "Edit" | "Write" => "\u{270E}",   // ✎
        "Read" => "\u{25C8}",              // ◈
        "Grep" | "Glob" => "\u{25CE}",    // ◎
        "Bash" => "\u{25B8}",             // ▸
        "Agent" => "\u{229B}",            // ⊛
        _ => "\u{25CF}",                  // ●
    }
}

pub fn tool_color(tool: &str, theme: &Theme) -> Color {
    match tool {
        "Edit" | "Write" => theme.success,
        "Read" | "Grep" | "Glob" => theme.secondary,
        "Bash" => theme.warning,
        "Agent" => theme.thinking,
        _ => theme.primary,
    }
}

pub fn status_indicator(status: &EntryStatus) -> (&'static str, Option<fn(&Theme) -> Color>) {
    match status {
        EntryStatus::Success => ("\u{2713} ", Some(|t: &Theme| t.success)),
        EntryStatus::Failure => ("\u{2717} ", Some(|t: &Theme| t.error)),
        EntryStatus::Warning => ("\u{26A0} ", Some(|t: &Theme| t.warning)),
        EntryStatus::Pending => ("\u{2026} ", None),
        EntryStatus::Neutral => ("", None),
    }
}

pub fn basename(path: &str) -> &str {
    let last_sep = path.rfind(['/', '\\']).map(|p| p + 1).unwrap_or(0);
    &path[last_sep..]
}

/// Make a file path relative to the project cwd
/// C:\Users\Admin\Documents\code\project\src\file.rs with cwd C:\Users\Admin\Documents\code\project
/// → src/file.rs
pub fn relative_path(file: &str, cwd: &str) -> String {
    if file.is_empty() || cwd.is_empty() {
        return file.to_string();
    }
    let f = file.replace('\\', "/").to_lowercase();
    let c = cwd.replace('\\', "/").to_lowercase().trim_end_matches('/').to_string();
    if f.starts_with(&c) {
        let rel = &file[c.len()..];
        let rel = rel.trim_start_matches(['/', '\\']);
        if rel.is_empty() { basename(file).to_string() } else { rel.replace('\\', "/") }
    } else {
        basename(file).to_string()
    }
}

pub fn fmt_duration_short(ms: u64) -> String {
    if ms == 0 { return String::new(); }
    let secs = ms as f64 / 1000.0;
    if secs < 60.0 { format!("{:.1}s", secs) }
    else if secs < 3600.0 { format!("{}m{:02}s", (secs / 60.0) as u64, (secs % 60.0) as u64) }
    else { format!("{}h{:02}m", (secs / 3600.0) as u64, ((secs % 3600.0) / 60.0) as u64) }
}

/// Truncate a string to fit within max_len, adding ... if truncated
fn truncate(s: &str, max_len: usize) -> String {
    if max_len < 4 { return String::new(); }
    if s.len() <= max_len { s.to_string() }
    else { format!("{}...", &s[..max_len - 3]) }
}

// ── Summary title ──

fn build_summary_title(entries: &[FeedEntry], width: u16) -> String {
    let total = entries.iter().filter(|e| !e.is_user_message).count();
    if total == 0 { return " Activity ".to_string(); }
    let edit_count = entries.iter().filter(|e| e.tool == "Edit" || e.tool == "Write").count();
    let read_count = entries.iter().filter(|e| e.tool == "Read" || e.tool == "Grep" || e.tool == "Glob").count();
    let bash_count = entries.iter().filter(|e| e.tool == "Bash").count();
    let unique_files: std::collections::HashSet<&str> = entries.iter()
        .filter(|e| !e.is_user_message && !e.file.is_empty())
        .map(|e| basename(&e.file)).collect();

    let full = format!(" Activity \u{2502} {} actions \u{2502} \u{270E}{} \u{25C8}{} \u{25B8}{} \u{2502} {} files ",
        total, edit_count, read_count, bash_count, unique_files.len());
    let medium = format!(" Activity \u{2502} {} actions \u{2502} \u{270E}{} \u{25C8}{} \u{25B8}{} ",
        total, edit_count, read_count, bash_count);
    let short = format!(" Activity \u{2502} {} actions ", total);

    let w = width as usize;
    if full.len() + 4 <= w { full }
    else if medium.len() + 4 <= w { medium }
    else if short.len() + 4 <= w { short }
    else { " Activity ".to_string() }
}

// ── Bash command → readable label ──

fn bash_label_from_cmd(cmd: &str) -> String {
    let cmd = cmd.trim();
    // Get the first token (the binary name)
    let first = cmd.split_whitespace().next().unwrap_or("");
    // Strip path prefixes: "/usr/bin/git" → "git", "C:\...\cloudflared.exe" → "cloudflared"
    let bin = first.rsplit(['/', '\\']).next().unwrap_or(first).trim_end_matches(".exe");

    match bin {
        "git" => {
            // Second token is the git subcommand
            let sub = cmd.split_whitespace().nth(1).unwrap_or("");
            match sub {
                "status" => "Git status".into(),
                "diff" => "Git diff".into(),
                "log" => "Git log".into(),
                "show" => "Git show".into(),
                "commit" => "Git commit".into(),
                "push" => "Git push".into(),
                "pull" => "Git pull".into(),
                "fetch" => "Git fetch".into(),
                "checkout" | "switch" => "Git checkout".into(),
                "branch" => "Git branch".into(),
                "add" => "Git add".into(),
                "stash" => "Git stash".into(),
                "merge" => "Git merge".into(),
                "rebase" => "Git rebase".into(),
                "clone" => "Git clone".into(),
                "reset" => "Git reset".into(),
                "tag" => "Git tag".into(),
                "remote" => "Git remote".into(),
                "rev-parse" => "Git rev-parse".into(),
                _ => format!("Git {}", sub),
            }
        }
        "grep" | "rg" => "Search".into(),
        "find" => "Find files".into(),
        "ls" | "dir" => "List files".into(),
        "cat" | "head" | "tail" | "less" => "Read file".into(),
        "mkdir" => "Create dir".into(),
        "rm" | "del" => "Remove".into(),
        "cp" | "copy" => "Copy".into(),
        "mv" | "move" => "Move".into(),
        "npm" | "pnpm" | "yarn" | "bun" => {
            let sub = cmd.split_whitespace().nth(1).unwrap_or("");
            match sub {
                "install" | "i" => format!("{} install", bin),
                "run" => {
                    let script = cmd.split_whitespace().nth(2).unwrap_or("");
                    format!("{} run {}", bin, script)
                }
                "test" | "t" => format!("{} test", bin),
                "build" => format!("{} build", bin),
                "dev" => format!("{} dev", bin),
                _ => format!("{} {}", bin, sub),
            }
        }
        "cargo" => {
            let sub = cmd.split_whitespace().nth(1).unwrap_or("");
            format!("Cargo {}", sub)
        }
        "python" | "python3" | "py" => "Python".into(),
        "node" => "Node".into(),
        "curl" | "wget" => "HTTP request".into(),
        "docker" => {
            let sub = cmd.split_whitespace().nth(1).unwrap_or("");
            format!("Docker {}", sub)
        }
        "powershell" | "pwsh" => "PowerShell".into(),
        "sed" | "awk" => "Transform".into(),
        "chmod" | "chown" | "icacls" => "Permissions".into(),
        "ssh" => "SSH".into(),
        "scp" | "rsync" => "File transfer".into(),
        "make" | "cmake" => "Build".into(),
        "rustc" => "Compile".into(),
        "gh" => {
            let sub = cmd.split_whitespace().nth(1).unwrap_or("");
            match sub {
                "pr" => "GitHub PR".into(),
                "issue" => "GitHub issue".into(),
                "run" => "GitHub Actions".into(),
                _ => format!("GitHub {}", sub),
            }
        }
        "echo" | "printf" => "Print".into(),
        "wc" => "Count".into(),
        "sort" | "uniq" => "Sort".into(),
        "tar" | "zip" | "unzip" | "gzip" => "Archive".into(),
        _ => "Shell".into(),
    }
}

// ── ActivityFeed widget ──

pub struct ActivityFeed {
    pub entries: Arc<Mutex<Vec<FeedEntry>>>,
}

impl ActivityFeed {
    pub fn new(entries: Arc<Mutex<Vec<FeedEntry>>>) -> Self {
        Self { entries }
    }
}

impl Widget for ActivityFeed {
    fn render(&self, frame: &mut Frame, area: Rect, primary_status: &SessionStatus, theme: &Theme, tick: u64) {
        let entries = self.entries.lock().unwrap();
        let title = build_summary_title(&entries, area.width);

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_set(theme.border_set())
            .border_style(Style::default().fg(theme.dimmed));

        let inner_height = area.height.saturating_sub(2) as usize;
        let inner_w = area.width.saturating_sub(2) as usize;

        // No blank separators — colored blocks create visual separation
        // Estimate ~2 lines per entry on average
        let max_visible = if inner_height == 0 { 0 } else { (inner_height + 1) / 2 };

        let start = entries.len().saturating_sub(max_visible);
        let visible = &entries[start..];
        let visible_count = visible.len();

        let mut lines: Vec<Line> = Vec::new();

        for (i, entry) in visible.iter().enumerate() {
            let ratio = if visible_count <= 1 { 1.0 } else { i as f32 / (visible_count - 1) as f32 };

            if entry.is_user_message {
                let sep_color = lerp_color(theme.dimmed, theme.text, ratio * 0.5);
                lines.push(Line::from(Span::styled(
                    " \u{2500}\u{2500} user \u{2500}\u{2500}",
                    Style::default().fg(sep_color),
                )));
                continue;
            }

            // ── Colors with fade (no bg — let ambient effects show through) ──
            let icon = tool_icon(&entry.tool);
            let tc = tool_color(&entry.tool, theme);

            let icon_color = lerp_color(theme.dimmed, tc, ratio.powf(0.5));
            let time_color = lerp_color(Color::Rgb(40, 40, 40), theme.dimmed, ratio);
            let label_color = lerp_color(theme.dimmed, tc, ratio.powf(0.6));
            let dim_color = lerp_color(Color::Rgb(40, 40, 40), theme.dimmed, ratio);
            let cmd_color = lerp_color(Color::Rgb(35, 35, 35), theme.dimmed, ratio * 0.7);
            let result_color = lerp_color(Color::Rgb(40, 40, 40), theme.dimmed, ratio * 0.9);

            let time_str = if entry.time.len() >= 5 { &entry.time[..5] } else { &entry.time };
            let file_base = relative_path(&entry.file, &entry.cwd);

            // Status indicator
            let (status_sym, status_color_fn) = status_indicator(&entry.status);
            let has_status_icon = !status_sym.is_empty() && !matches!(entry.status, EntryStatus::Pending | EntryStatus::Neutral);
            let s_color = status_color_fn.map(|f| f(theme)).unwrap_or(theme.dimmed);

            // ── Extract per-tool data ──
            let (label, compact_info, target, result_text): (String, String, String, String) = match entry.tool.as_str() {
                "Read" => {
                    ("Read".into(), entry.result.clone(), file_base.clone(), String::new())
                }
                "Edit" => {
                    let info = format!("{}{}", entry.detail, if has_status_icon { format!("  {}", status_sym.trim()) } else { String::new() });
                    ("Edit".into(), info, file_base.clone(), String::new())
                }
                "Write" => {
                    let info = format!("{}{}", entry.detail, if has_status_icon { format!("  {}", status_sym.trim()) } else { String::new() });
                    ("Create".into(), info, file_base.clone(), String::new())
                }
                "Bash" => {
                    let cmd_display = if !entry.detail.is_empty() {
                        let d = entry.detail.clone();
                        if let Some(pos) = d.find(" && ") {
                            if d[..pos].starts_with("cd ") { d[pos + 4..].to_string() } else { d }
                        } else { d }
                    } else { String::new() };
                    let desc = if !entry.description.is_empty() {
                        entry.description.clone()
                    } else {
                        bash_label_from_cmd(&cmd_display)
                    };
                    let status_str = if has_status_icon { status_sym.trim().to_string() } else { String::new() };
                    (desc, status_str, cmd_display, entry.result.clone())
                }
                "Grep" => {
                    ("Search".into(), entry.result.clone(), entry.detail.clone(), String::new())
                }
                "Glob" => {
                    ("Find".into(), entry.result.clone(), entry.detail.clone(), String::new())
                }
                "Agent" => {
                    let status_str = if has_status_icon { status_sym.trim().to_string() } else { String::new() };
                    ("Agent".into(), status_str, entry.detail.clone(), entry.result.clone())
                }
                "TaskCreate" => ("Task created".into(), String::new(), entry.detail.clone(), String::new()),
                "TaskUpdate" => ("Task updated".into(), String::new(), entry.detail.clone(), String::new()),
                "WebSearch" => ("Web search".into(), entry.result.clone(), entry.detail.clone(), String::new()),
                "WebFetch" => ("Fetch".into(), entry.result.clone(), entry.detail.clone(), String::new()),
                "Skill" => ("Skill".into(), String::new(), entry.detail.clone(), String::new()),
                _ => {
                    let desc = if entry.description.is_empty() { entry.tool.clone() } else { entry.description.clone() };
                    (desc, String::new(), entry.detail.clone(), String::new())
                }
            };

            // ── Helper macro: collect spans into a line (no bg padding) ──
            macro_rules! pad_line {
                ($spans:expr, $w:expr) => {{
                    Line::from($spans)
                }};
            }

            // ── Line 1: icon + time + label + compact_info ──
            let mut l1: Vec<Span> = vec![
                Span::styled(format!(" {}", icon), Style::default().fg(icon_color)),
                Span::styled(format!(" {}", time_str), Style::default().fg(time_color)),
                Span::styled(format!("  {}", label), Style::default().fg(label_color)),
            ];
            if !compact_info.is_empty() {
                let ci_color = if compact_info.contains('\u{2713}') || compact_info.contains('\u{2717}') {
                    s_color
                } else {
                    dim_color
                };
                l1.push(Span::styled(format!("  {}", compact_info), Style::default().fg(ci_color)));
            }

            // Truncate line 1 to fit
            let total_w: usize = l1.iter().map(|s| s.content.len()).sum();
            if total_w > inner_w && l1.len() > 1 {
                let last_idx = l1.len() - 1;
                let last_len = l1[last_idx].content.len();
                let overflow = total_w - inner_w;
                if last_len > overflow + 3 {
                    let new_len = last_len - overflow - 3;
                    let style = l1[last_idx].style;
                    l1[last_idx] = Span::styled(format!("{}...", &l1[last_idx].content[..new_len]), style);
                }
            }
            lines.push(pad_line!(l1, inner_w));

            // ── Line 2: target/command ──
            let indent = "          ";
            if !target.is_empty() {
                let max_target = inner_w.saturating_sub(indent.len());
                let display = if target.len() > max_target && max_target > 3 {
                    format!("{}...", &target[..max_target - 3])
                } else {
                    target
                };
                lines.push(pad_line!(vec![
                    Span::styled(format!("{}{}", indent, display), Style::default().fg(cmd_color)),
                ], inner_w));
            }

            // ── Line 3: result output ──
            if !result_text.is_empty() {
                let max_result = inner_w.saturating_sub(indent.len());
                let display = if result_text.len() > max_result && max_result > 3 {
                    format!("{}...", &result_text[..max_result - 3])
                } else {
                    result_text
                };
                lines.push(pad_line!(vec![
                    Span::styled(format!("{}{}", indent, display), Style::default().fg(result_color)),
                ], inner_w));
            }
        }

        // Trim to fit inner_height
        if lines.len() > inner_height {
            let skip = lines.len() - inner_height;
            lines.drain(0..skip);
        }

        let paragraph = if lines.is_empty() {
            Paragraph::new("  Waiting for activity...").style(Style::default().fg(theme.dimmed))
        } else {
            Paragraph::new(lines)
        };

        frame.render_widget(paragraph.block(block), area);
    }
}
