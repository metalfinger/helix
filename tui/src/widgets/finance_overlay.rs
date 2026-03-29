use crate::memory_state::ExplorerData;
use crate::theme::Theme;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub struct FinanceState {
    pub selected: usize,
    pub scroll: u16,
    pub view: usize, // 0 = dashboard (charts), 1 = text detail
}

impl FinanceState {
    pub fn new() -> Self {
        Self { selected: 0, scroll: 0, view: 0 }
    }
    pub fn reset(&mut self) {
        self.selected = 0;
        self.scroll = 0;
        // keep view preference across opens
    }
}

fn fmt_inr(amount: i64) -> String {
    if amount >= 10_000_000 {
        format!("{:.2} Cr", amount as f64 / 10_000_000.0)
    } else if amount >= 100_000 {
        format!("{:.2}L", amount as f64 / 100_000.0)
    } else if amount >= 1_000 {
        format!("{}K", amount / 1_000)
    } else {
        amount.to_string()
    }
}

fn fmt_full(amount: i64) -> String {
    if amount >= 10_000_000 {
        format!("Rs {:.2} Cr", amount as f64 / 10_000_000.0)
    } else if amount >= 100_000 {
        format!("Rs {:.2} L", amount as f64 / 100_000.0)
    } else {
        format!("Rs {}", format_indian(amount))
    }
}

fn format_indian(n: i64) -> String {
    let s = n.to_string();
    if s.len() <= 3 { return s; }
    let (first, rest) = s.split_at(s.len() - 3);
    let mut result = String::new();
    let chars: Vec<char> = first.chars().rev().collect();
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && i % 2 == 0 { result.push(','); }
        result.push(*c);
    }
    result = result.chars().rev().collect();
    format!("{},{}", result, rest)
}

// Block chars for bar chart: ▁▂▃▄▅▆▇█
const BAR_CHARS: [char; 8] = ['\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];

fn bar_char(value: f64, max: f64, height: usize) -> char {
    if max <= 0.0 || value <= 0.0 { return ' '; }
    let ratio = (value / max).min(1.0);
    let level = (ratio * height as f64 * 8.0) as usize;
    if level == 0 { return ' '; }
    BAR_CHARS[(level - 1).min(7)]
}

pub fn render_finance_overlay(
    frame: &mut Frame,
    data: &ExplorerData,
    state: &FinanceState,
    theme: &Theme,
) {
    let area = frame.area();
    frame.render_widget(Clear, area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.dimmed))
        .style(Style::default().bg(Color::Black));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // header
            Constraint::Min(0),    // content
            Constraint::Length(1),  // sticky footer totals
            Constraint::Length(1),  // keybindings
        ])
        .split(inner);

    let s = &data.finance_summary;
    let payments = &data.payments;
    let target = s.freelance_target;

    // ═══════════════ HEADER ═══════════════
    let view_label = if state.view == 0 { "Dashboard" } else { "Detail" };
    let header = Line::from(vec![
        Span::styled(" FINANCES ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(format!("[{}]", view_label), Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("   Salary {}/mo", fmt_inr(s.salary_amount)),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(
            format!("   Goal {}/mo", fmt_inr(target)),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled("   [Tab] switch", Style::default().fg(Color::Rgb(50, 50, 50))),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[0]);

    // ═══════════════ MAIN CONTENT ═══════════════
    let mut lines: Vec<Line> = Vec::new();
    let w = inner.width as usize;

    // ══════ VIEW 1: TEXT DETAIL ══════
    if state.view == 1 && target > 0 {
        let locked = s.freelance_locked;
        let ahead = s.freelance_ahead_behind;
        let runway = s.freelance_runway_months;
        let req_rate = s.freelance_required_rate;
        let pct = s.freelance_completion_pct;
        let yearly_target = s.freelance_target_yearly;
        let yearly_salary = s.salary_amount * 12;

        lines.push(Line::from(Span::styled(
            " -- Freelance Year Target (Apr 2026 - Mar 2027) --",
            Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        // Status
        let (status_text, status_color) = if ahead >= 0 {
            (format!("AHEAD by {}", fmt_full(ahead)), Color::Green)
        } else {
            (format!("BEHIND by {}", fmt_full(-ahead)), Color::Red)
        };
        lines.push(Line::from(vec![
            Span::styled("  Status       ", Style::default().fg(Color::DarkGray)),
            Span::styled(status_text, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Locked in    ", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_full(locked), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  ({:.0}% of {})", pct, fmt_full(yearly_target)), Style::default().fg(Color::DarkGray)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Breakdown    ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} earned", fmt_full(s.freelance_earned)), Style::default().fg(Color::Green)),
            Span::styled(format!(" + {} pipeline", fmt_full(s.freelance_pipeline)), Style::default().fg(Color::Cyan)),
            if s.monthly_owed > 0 {
                Span::styled(format!(" + {} owed", fmt_full(s.monthly_owed)), Style::default().fg(Color::Magenta))
            } else { Span::styled("", Style::default()) },
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Runway       ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if runway > 0.0 { format!("{:.1} months buffer", runway) } else { "none".to_string() },
                Style::default().fg(if runway > 0.0 { Color::Green } else { Color::Red }),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Run rate     ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}/mo needed", fmt_full(req_rate)), Style::default().fg(if req_rate <= target { Color::Green } else { Color::Yellow })),
            Span::styled(
                if req_rate < target { "  (surplus helping)" } else if req_rate == target { "  (on target)" } else { "  (need to catch up)" },
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(""));

        // Pipeline details
        let pipeline_total: i64 = payments.iter().filter(|p| !p.is_salary && p.status == "pending").map(|p| p.amount).sum();
        let owed_total: i64 = payments.iter().filter(|p| p.status == "owed").map(|p| p.amount).sum();
        let months_covered = if target > 0 { pipeline_total as f64 / target as f64 } else { 0.0 };
        let remaining_gap = (yearly_target - locked).max(0);
        let months_to_fill = if target > 0 { (remaining_gap as f64 / target as f64).ceil() as i64 } else { 0 };

        lines.push(Line::from(vec![
            Span::styled("  Pipeline     ", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_full(pipeline_total), Style::default().fg(Color::Cyan)),
            Span::styled(format!("  = {:.1} months covered", months_covered), Style::default().fg(Color::DarkGray)),
        ]));
        if owed_total > 0 {
            lines.push(Line::from(vec![
                Span::styled("  Owed         ", Style::default().fg(Color::DarkGray)),
                Span::styled(fmt_full(owed_total), Style::default().fg(Color::Magenta)),
                Span::styled("  expected from people", Style::default().fg(Color::DarkGray)),
            ]));
        }
        if remaining_gap > 0 {
            lines.push(Line::from(vec![
                Span::styled("  Gap          ", Style::default().fg(Color::DarkGray)),
                Span::styled(fmt_full(remaining_gap), Style::default().fg(Color::Yellow)),
                Span::styled(format!("  needed ({} months to fill)", months_to_fill), Style::default().fg(Color::DarkGray)),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Year total   ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("Salary {} + Freelance {} = ", fmt_full(yearly_salary), fmt_full(yearly_target)), Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_full(yearly_salary + yearly_target), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]));

        // Monthly flow (condensed)
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " -- Monthly Flow --",
            Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
        )));
        let mut empty_start: Option<String> = None;
        let mut empty_count: usize = 0;
        let mut empty_last = String::new();
        for fc in &s.forecast {
            let has_data = fc.freelance_confirmed > 0 || fc.received > 0;
            if has_data {
                if empty_count > 0 {
                    let range = if empty_count == 1 { empty_start.unwrap_or_default() } else { format!("{} - {}", empty_start.unwrap_or_default(), empty_last) };
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {}", range), Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("   {}/mo  {} months, need clients", fmt_full(s.salary_amount + target), empty_count), Style::default().fg(Color::Rgb(70, 70, 70))),
                    ]));
                    empty_start = None;
                    empty_count = 0;
                }
                let mut spans = vec![
                    Span::styled(format!("  {:<10}", fc.label), Style::default().fg(Color::White)),
                    Span::styled(format!("{:<12}", fmt_full(fc.total)), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ];
                if fc.freelance_confirmed > 0 {
                    spans.push(Span::styled(format!("{} locked", fmt_full(fc.freelance_confirmed)), Style::default().fg(Color::Cyan)));
                }
                if fc.freelance_gap > 0 {
                    spans.push(Span::styled(format!("  need {}", fmt_full(fc.freelance_gap)), Style::default().fg(Color::Yellow)));
                }
                lines.push(Line::from(spans));
            } else {
                if empty_start.is_none() { empty_start = Some(fc.label.clone()); }
                empty_last = fc.label.clone();
                empty_count += 1;
            }
        }
        if empty_count > 0 {
            let range = if empty_count == 1 { empty_start.unwrap_or_default() } else { format!("{} - {}", empty_start.unwrap_or_default(), empty_last) };
            lines.push(Line::from(vec![
                Span::styled(format!("  {}", range), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("   {}/mo  {} months, need clients", fmt_full(s.salary_amount + target), empty_count), Style::default().fg(Color::Rgb(70, 70, 70))),
            ]));
        }
        lines.push(Line::from(""));
    }

    // ══════ VIEW 0: DASHBOARD (charts + cards) ══════
    if state.view == 0 && target > 0 {
        let locked = s.freelance_locked;
        let ahead = s.freelance_ahead_behind;
        let runway = s.freelance_runway_months;
        let req_rate = s.freelance_required_rate;
        let pct = s.freelance_completion_pct;
        let yearly_target = s.freelance_target_yearly;

        // ── Full-width progress bar ──
        lines.push(Line::from(""));
        let bar_w = w.saturating_sub(4);
        let fill = ((pct / 100.0) * bar_w as f64).min(bar_w as f64) as usize;
        let target_pos = if s.freelance_months_elapsed > 0 {
            ((s.freelance_months_elapsed as f64 / 12.0) * bar_w as f64) as usize
        } else { 0 };

        let mut bar_spans: Vec<Span> = vec![Span::styled("  ", Style::default())];
        for i in 0..bar_w {
            let ch;
            let color;
            if i == target_pos && target_pos > 0 {
                ch = '\u{2502}'; // │ target marker
                color = if fill >= target_pos { Color::Green } else { Color::Red };
            } else if i < fill {
                ch = '\u{2593}'; // ▓ filled
                color = Color::Cyan;
            } else {
                ch = '\u{2591}'; // ░ empty
                color = Color::Rgb(35, 35, 35);
            }
            bar_spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
        }
        lines.push(Line::from(bar_spans));

        // Progress label centered under bar
        let label = format!(
            "{}  {:.0}% of {}",
            fmt_full(locked), pct, fmt_full(yearly_target)
        );
        let pad = (w.saturating_sub(label.len())) / 2;
        lines.push(Line::from(vec![
            Span::styled(" ".repeat(pad), Style::default()),
            Span::styled(label, Style::default().fg(Color::DarkGray)),
        ]));
        lines.push(Line::from(""));

        // ── Metric cards ──
        // Calculate card widths (3 cards, evenly spaced)
        let card_w = (w.saturating_sub(8)) / 3;

        // Card 1: LOCKED IN
        let c1_title = " LOCKED IN";
        let c1_val = fmt_full(locked);
        let c1_sub = format!(
            "{} earned + {} pipeline",
            fmt_inr(s.freelance_earned), fmt_inr(s.freelance_pipeline)
        );

        // Card 2: RUNWAY
        let c2_title = " RUNWAY";
        let c2_val = if runway > 0.0 { format!("{:.1} months", runway) } else { "0".to_string() };
        let c2_sub = if ahead >= 0 {
            format!("{} ahead", fmt_full(ahead))
        } else {
            format!("{} behind", fmt_full(-ahead))
        };

        // Card 3: RUN RATE
        let c3_title = " RUN RATE";
        let c3_val = format!("{}/mo", fmt_full(req_rate));
        let c3_sub = if req_rate < target {
            "surplus helping".to_string()
        } else if req_rate == target {
            "on target".to_string()
        } else {
            "catch up needed".to_string()
        };

        // Top border
        let top = format!(
            "  {}{} {}{} {}{}",
            "\u{250C}", "\u{2500}".repeat(card_w),
            "\u{250C}", "\u{2500}".repeat(card_w),
            "\u{250C}", "\u{2500}".repeat(card_w),
        );
        lines.push(Line::from(Span::styled(top, Style::default().fg(Color::DarkGray))));

        // Title row
        let ahead_color = if ahead >= 0 { Color::Green } else { Color::Red };
        let rate_color = if req_rate <= target { Color::Green } else { Color::Yellow };
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:<w$}", c1_title, w = card_w), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(" \u{2502}", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:<w$}", c2_title, w = card_w), Style::default().fg(ahead_color).add_modifier(Modifier::BOLD)),
            Span::styled(" \u{2502}", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:<w$}", c3_title, w = card_w), Style::default().fg(rate_color).add_modifier(Modifier::BOLD)),
        ]));

        // Value row
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}", Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {:<w$}", c1_val, w = card_w - 1), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(" \u{2502}", Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {:<w$}", c2_val, w = card_w - 1), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(" \u{2502}", Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {:<w$}", c3_val, w = card_w - 1), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]));

        // Sub row
        lines.push(Line::from(vec![
            Span::styled("  \u{2502}", Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {:<w$}", c1_sub, w = card_w - 1), Style::default().fg(Color::DarkGray)),
            Span::styled(" \u{2502}", Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {:<w$}", c2_sub, w = card_w - 1), Style::default().fg(ahead_color)),
            Span::styled(" \u{2502}", Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {:<w$}", c3_sub, w = card_w - 1), Style::default().fg(Color::DarkGray)),
        ]));

        // Bottom border
        let bot = format!(
            "  {}{} {}{} {}{}",
            "\u{2514}", "\u{2500}".repeat(card_w),
            "\u{2514}", "\u{2500}".repeat(card_w),
            "\u{2514}", "\u{2500}".repeat(card_w),
        );
        lines.push(Line::from(Span::styled(bot, Style::default().fg(Color::DarkGray))));

        // ── 12-Month Bar Chart ──
        lines.push(Line::from(""));
        if !s.forecast.is_empty() {
            // Scale chart to ~1.5x target so normal months fill nicely.
            // Outlier months like April clip at full height — that's fine.
            let target_monthly = (s.salary_amount + target) as f64;
            let max_val = (target_monthly * 1.5).max(1.0);

            let col_w = ((w.saturating_sub(6)) / s.forecast.len()).max(3);
            let chart_h = 5; // 5 rows of bar height for better resolution

            // Build chart rows top to bottom
            for row in (0..chart_h).rev() {
                let mut spans: Vec<Span> = vec![Span::styled("  ", Style::default())];
                // Y-axis label on first and last row
                if row == chart_h - 1 {
                    spans.push(Span::styled(format!("{:<4}", fmt_inr(max_val as i64)), Style::default().fg(Color::DarkGray)));
                } else if row == 0 {
                    spans.push(Span::styled("0   ", Style::default().fg(Color::DarkGray)));
                } else {
                    spans.push(Span::styled("    ", Style::default()));
                }

                for fc in &s.forecast {
                    let confirmed = (fc.salary + fc.freelance_confirmed) as f64;
                    let total = fc.total as f64;

                    // For this row, what fraction is filled?
                    let row_bottom = row as f64 / chart_h as f64;
                    let row_top = (row + 1) as f64 / chart_h as f64;

                    let confirmed_ratio = (confirmed / max_val).min(1.0);
                    let total_ratio = (total / max_val).min(1.0);

                    let mut cell = String::new();
                    let mut cell_color = Color::Rgb(30, 30, 30);

                    if confirmed_ratio > row_bottom {
                        // This row has confirmed data
                        if confirmed_ratio >= row_top {
                            cell = "\u{2588}".repeat(col_w.saturating_sub(1)); // full block
                        } else {
                            let partial = ((confirmed_ratio - row_bottom) / (row_top - row_bottom) * 8.0) as usize;
                            let ch = BAR_CHARS[partial.min(7)];
                            cell = ch.to_string().repeat(col_w.saturating_sub(1));
                        }
                        if fc.freelance_confirmed > 0 {
                            cell_color = Color::Cyan;
                        } else {
                            cell_color = Color::Rgb(50, 80, 80); // salary only - dimmer
                        }
                    } else if total_ratio > row_bottom {
                        // This row has target data but not confirmed
                        if total_ratio >= row_top {
                            cell = "\u{2592}".repeat(col_w.saturating_sub(1)); // medium shade
                        } else {
                            let partial = ((total_ratio - row_bottom) / (row_top - row_bottom) * 8.0) as usize;
                            let ch = BAR_CHARS[partial.min(7)];
                            cell = ch.to_string().repeat(col_w.saturating_sub(1));
                        }
                        cell_color = Color::Rgb(50, 50, 30); // target - dark yellow
                    } else {
                        cell = " ".repeat(col_w.saturating_sub(1));
                    }

                    spans.push(Span::styled(cell, Style::default().fg(cell_color)));
                    spans.push(Span::styled(" ", Style::default()));
                }
                lines.push(Line::from(spans));
            }

            // X-axis: month labels
            let mut label_spans: Vec<Span> = vec![Span::styled("      ", Style::default())];
            for fc in &s.forecast {
                let short = if fc.label.len() >= 3 { &fc.label[..3] } else { &fc.label };
                let pad = col_w.saturating_sub(short.len());
                let color = if fc.freelance_confirmed > 0 { Color::Cyan } else { Color::DarkGray };
                label_spans.push(Span::styled(short.to_string(), Style::default().fg(color)));
                label_spans.push(Span::styled(" ".repeat(pad.max(1)), Style::default()));
            }
            lines.push(Line::from(label_spans));

            // Legend
            lines.push(Line::from(vec![
                Span::styled("      ", Style::default()),
                Span::styled("\u{2588}", Style::default().fg(Color::Cyan)),
                Span::styled(" confirmed  ", Style::default().fg(Color::DarkGray)),
                Span::styled("\u{2592}", Style::default().fg(Color::Rgb(50, 50, 30))),
                Span::styled(" target  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("target line: {}/mo", fmt_inr(target)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        // ── Text Summary ──
        lines.push(Line::from(""));
        let pipeline_confirmed: i64 = payments.iter()
            .filter(|p| !p.is_salary && p.status == "pending")
            .map(|p| p.amount).sum();
        let owed_sum: i64 = payments.iter()
            .filter(|p| p.status == "owed")
            .map(|p| p.amount).sum();
        let months_covered = if target > 0 { pipeline_confirmed as f64 / target as f64 } else { 0.0 };
        let remaining_gap = (yearly_target - locked).max(0);
        let months_to_fill = if target > 0 { (remaining_gap as f64 / target as f64).ceil() as i64 } else { 0 };

        lines.push(Line::from(vec![
            Span::styled("  Confirmed  ", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_full(pipeline_confirmed), Style::default().fg(Color::Cyan)),
            Span::styled(format!(" pipeline = {:.1} months covered", months_covered), Style::default().fg(Color::DarkGray)),
        ]));
        if owed_sum > 0 {
            lines.push(Line::from(vec![
                Span::styled("  Owed       ", Style::default().fg(Color::DarkGray)),
                Span::styled(fmt_full(owed_sum), Style::default().fg(Color::Magenta)),
                Span::styled(" expected from people", Style::default().fg(Color::DarkGray)),
            ]));
        }
        if remaining_gap > 0 {
            lines.push(Line::from(vec![
                Span::styled("  Gap        ", Style::default().fg(Color::DarkGray)),
                Span::styled(fmt_full(remaining_gap), Style::default().fg(Color::Yellow)),
                Span::styled(format!(" needed ({} months to fill)", months_to_fill), Style::default().fg(Color::DarkGray)),
            ]));
        }

        let yearly_salary = s.salary_amount * 12;
        lines.push(Line::from(vec![
            Span::styled("  Year       ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("Salary {} + Freelance {} = ", fmt_full(yearly_salary), fmt_full(yearly_target)),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(fmt_full(yearly_salary + yearly_target), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]));

        lines.push(Line::from(""));
    }

    // ═══════════════ PAYMENT LIST ═══════════════
    let sections: [(& str, &str, Color); 4] = [
        ("overdue", "OVERDUE", Color::Red),
        ("owed", "OWED TO YOU", Color::Magenta),
        ("pending", "PIPELINE", Color::Cyan),
        ("paid", "RECEIVED", Color::Green),
    ];

    // Salary (compact)
    if payments.iter().any(|p| p.is_salary) {
        lines.push(Line::from(vec![
            Span::styled("  \u{25CF} Salary ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{}/mo", fmt_full(s.salary_amount)),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ({})", s.salary_source),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    let mut item_idx: usize = 0;
    for (status_key, section_label, section_color) in &sections {
        let section_payments: Vec<_> = payments.iter()
            .filter(|p| !p.is_salary && p.status == *status_key)
            .collect();
        if section_payments.is_empty() { continue; }

        let section_total: i64 = section_payments.iter().map(|p| p.amount).sum();
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} ", section_label),
                Style::default().fg(*section_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                fmt_full(section_total),
                Style::default().fg(*section_color),
            ),
        ]));

        for payment in &section_payments {
            let is_selected = item_idx == state.selected;
            let marker = if is_selected { "\u{25B8} " } else { "  " };
            let base_style = if is_selected {
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };

            // Urgency-based dimming: closer to due = brighter
            let urgency_style = if let Some(dl) = payment.days_left {
                if dl < 0 {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else if dl <= 3 {
                    Style::default().fg(Color::Red)
                } else if dl <= 7 {
                    Style::default().fg(Color::Yellow)
                } else if dl <= 14 {
                    base_style
                } else {
                    Style::default().fg(Color::Rgb(120, 120, 120))
                }
            } else if payment.status == "paid" {
                Style::default().fg(Color::Rgb(80, 80, 80))
            } else {
                base_style
            };

            let max_name = (area.width as usize).saturating_sub(45);
            let name = if payment.name.chars().count() > max_name && max_name > 3 {
                let truncated: String = payment.name.chars().take(max_name - 3).collect();
                format!("{}...", truncated)
            } else {
                payment.name.clone()
            };

            let mut spans = vec![
                Span::styled(format!("  {}", marker), urgency_style),
                Span::styled(name, urgency_style),
            ];

            // Amount — always visible and bold
            spans.push(Span::styled(
                format!("  {}", fmt_full(payment.amount)),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ));

            // Time
            if payment.status == "paid" {
                if let Some(ref pd) = payment.paid_date {
                    spans.push(Span::styled(format!("  {}", pd), Style::default().fg(Color::Rgb(60, 60, 60))));
                }
            } else if let Some(dl) = payment.days_left {
                let (time_str, time_color) = if dl < 0 {
                    (format!("  {}d overdue", -dl), Color::Red)
                } else if dl == 0 {
                    ("  TODAY".to_string(), Color::Red)
                } else if dl <= 7 {
                    (format!("  {}d", dl), Color::Yellow)
                } else {
                    (format!("  {}d", dl), Color::Rgb(80, 80, 80))
                };
                spans.push(Span::styled(time_str, Style::default().fg(time_color)));
            }

            lines.push(Line::from(spans));
            item_idx += 1;
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No payments tracked. Use helix_add_payment or helix_set_salary to start.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Scroll
    let content_height = chunks[1].height as usize;
    let scroll = if lines.len() > content_height {
        let max = (lines.len() - content_height) as u16;
        state.scroll.min(max)
    } else { 0 };

    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), chunks[1]);

    // ═══════════════ STICKY TOTALS ═══════════════
    let pipeline_total: i64 = payments.iter().filter(|p| !p.is_salary && p.status == "pending").map(|p| p.amount).sum();
    let owed_total: i64 = payments.iter().filter(|p| p.status == "owed").map(|p| p.amount).sum();
    let yearly_total = s.salary_amount * 12 + s.freelance_target_yearly;

    let totals = Line::from(vec![
        Span::styled(" Pipeline ", Style::default().fg(Color::Cyan)),
        Span::styled(fmt_full(pipeline_total), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("  Owed ", Style::default().fg(Color::Magenta)),
        Span::styled(fmt_full(owed_total), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Span::styled("  YTD ", Style::default().fg(Color::Green)),
        Span::styled(fmt_full(s.yearly_received), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("  Year {}", fmt_full(yearly_total)),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(totals), chunks[2]);

    // ═══════════════ KEYBINDINGS ═══════════════
    let footer = " [jk] navigate  [m] mark paid  [W] memory  [Esc] close ";
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(footer, Style::default().fg(theme.dimmed)))),
        chunks[3],
    );
}
