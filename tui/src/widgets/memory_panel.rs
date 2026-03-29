use crate::memory_state::{MemoryWorldState, MemoryHealth};
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

/// Entry point — renders the Memory panel into the given area.
pub fn render_memory_panel(
    frame: &mut Frame,
    area: Rect,
    world_state: &Option<MemoryWorldState>,
    health: &Option<MemoryHealth>,
    theme: &Theme,
    _tick: u64,
) {
    let block = Block::default()
        .title(" Memory ")
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.primary));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    match world_state {
        None => {
            let msg = Paragraph::new(Line::from(Span::styled(
                "Memory: not connected",
                Style::default().fg(theme.dimmed),
            )));
            frame.render_widget(msg, inner);
        }
        Some(ws) => {
            render_sections(frame, inner, ws, health, theme);
        }
    }
}

// ── Internal rendering ──────────────────────────────────────────────────────

fn render_sections(
    frame: &mut Frame,
    area: Rect,
    ws: &MemoryWorldState,
    health: &Option<MemoryHealth>,
    theme: &Theme,
) {
    if area.height == 0 {
        return;
    }

    let has_urgent = !ws.sections.urgent.is_empty();
    let has_deadlines = !ws.sections.deadlines.is_empty();
    let has_actions = !ws.sections.pending_actions.is_empty();
    let has_projects = !ws.sections.projects_aeos.is_empty()
        || !ws.sections.projects_metalfinger.is_empty()
        || !ws.sections.projects_personal.is_empty();
    let has_decisions = !ws.sections.recent_decisions.is_empty();
    let has_waiting = !ws.sections.waiting_on.is_empty();
    let has_team = !ws.sections.team_pulse.is_empty();
    let has_stale = !ws.sections.stale_threads.is_empty();
    let has_dl_or_act = has_deadlines || has_actions;
    let has_dec_or_wait = has_decisions || has_waiting;
    let has_team_or_stale = has_team || has_stale;

    // Tasks from explorer data (task entities show as "pending" status)
    let tasks: Vec<&crate::memory_state::ExplorerActionCard> = ws.explorer.actions.iter()
        .filter(|a| a.status == "pending" || a.status == "in_progress")
        .collect();
    let has_tasks = !tasks.is_empty();

    let urgent_h: u16 = if has_urgent {
        1 + ws.sections.urgent.len().min(3) as u16
    } else {
        0
    };
    let tasks_h: u16 = if has_tasks {
        1 + tasks.len().min(5) as u16
    } else {
        0
    };
    let has_waiting_standalone = !ws.sections.waiting_on.is_empty();
    let waiting_h: u16 = if has_waiting_standalone {
        1 + ws.sections.waiting_on.len().min(3) as u16
    } else {
        0
    };
    let dl_act_h: u16 = if has_dl_or_act {
        1 + ws.sections.deadlines.len().max(ws.sections.pending_actions.len()).min(4) as u16
    } else {
        0
    };
    let all_projects: Vec<_> = ws.sections.projects_aeos.iter()
        .chain(ws.sections.projects_metalfinger.iter())
        .chain(ws.sections.projects_personal.iter())
        .collect();
    let proj_h: u16 = if has_projects {
        1 + all_projects.len().min(4) as u16
    } else {
        0
    };
    let dec_wait_h: u16 = if has_dec_or_wait {
        1 + ws.sections.recent_decisions.len().max(ws.sections.waiting_on.len()).min(3) as u16
    } else {
        0
    };
    let team_stale_h: u16 = if has_team_or_stale {
        1 + ws.sections.team_pulse.len().max(ws.sections.stale_threads.len()).min(3) as u16
    } else {
        0
    };
    let footer_h: u16 = 1;

    let total_needed = urgent_h + tasks_h + waiting_h + dl_act_h + proj_h + dec_wait_h + team_stale_h + footer_h;

    let mut constraints: Vec<Constraint> = Vec::new();
    if has_urgent { constraints.push(Constraint::Length(urgent_h)); }
    if has_tasks { constraints.push(Constraint::Length(tasks_h)); }
    if has_waiting_standalone { constraints.push(Constraint::Length(waiting_h)); }
    if has_dl_or_act { constraints.push(Constraint::Length(dl_act_h)); }
    if has_projects { constraints.push(Constraint::Length(proj_h)); }
    if has_dec_or_wait { constraints.push(Constraint::Length(dec_wait_h)); }
    if has_team_or_stale { constraints.push(Constraint::Length(team_stale_h)); }
    // Spacer absorbs slack when panel is taller than content
    if total_needed < area.height.saturating_sub(footer_h) {
        constraints.push(Constraint::Min(0));
    }
    constraints.push(Constraint::Length(footer_h));

    let rows = Layout::vertical(constraints.clone()).split(area);

    let mut idx: usize = 0;

    if has_urgent {
        render_urgent(frame, rows[idx], &ws.sections.urgent, theme, area.width);
        idx += 1;
    }
    if has_tasks {
        render_tasks(frame, rows[idx], &tasks, theme, area.width);
        idx += 1;
    }
    if has_waiting_standalone {
        render_waiting_standalone(frame, rows[idx], &ws.sections.waiting_on, theme, area.width);
        idx += 1;
    }
    if has_dl_or_act {
        render_deadlines_actions(frame, rows[idx], &ws.sections.deadlines, &ws.sections.pending_actions, theme);
        idx += 1;
    }
    if has_projects {
        render_projects(frame, rows[idx], &all_projects, theme, area.width);
        idx += 1;
    }
    if has_dec_or_wait {
        render_decisions_waiting(frame, rows[idx], &ws.sections.recent_decisions, &ws.sections.waiting_on, theme);
        idx += 1;
    }
    if has_team_or_stale {
        render_team_stale(frame, rows[idx], &ws.sections.team_pulse, &ws.sections.stale_threads, theme);
        idx += 1;
    }
    // Skip the Min(0) spacer row if we added one
    if total_needed < area.height.saturating_sub(footer_h) {
        idx += 1;
    }

    let footer_area = rows[idx];
    render_footer(frame, footer_area, ws, health, theme);
}

// ── Section renderers ────────────────────────────────────────────────────────

fn render_urgent(
    frame: &mut Frame,
    area: Rect,
    urgent: &[crate::memory_state::UrgentItem],
    theme: &Theme,
    width: u16,
) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        " [!] URGENT",
        Style::default().fg(theme.error).add_modifier(Modifier::BOLD),
    )));
    for item in urgent.iter().take(3) {
        let text = truncate(&item.description, width.saturating_sub(4) as usize);
        lines.push(Line::from(vec![
            Span::styled("  * ", Style::default().fg(theme.error)),
            Span::styled(text, Style::default().fg(theme.error)),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_tasks(
    frame: &mut Frame,
    area: Rect,
    tasks: &[&crate::memory_state::ExplorerActionCard],
    theme: &Theme,
    width: u16,
) {
    let mut lines: Vec<Line> = Vec::new();
    let max_w = width.saturating_sub(4) as usize;
    let count = tasks.len();
    lines.push(Line::from(Span::styled(
        format!(" TASKS ({})", count),
        Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
    )));
    for task in tasks.iter().take(5) {
        let deadline_str = match &task.deadline {
            Some(d) if !d.is_empty() => format!(" [{}]", d),
            _ => String::new(),
        };
        let proj_str = if task.project.is_empty() {
            String::new()
        } else {
            format!(" ({})", task.project)
        };
        let dot_color = if task.status == "in_progress" {
            theme.thinking
        } else if task.deadline.is_some() {
            theme.warning
        } else {
            theme.secondary
        };
        let label = truncate(&format!("{}{}{}", task.description, proj_str, deadline_str), max_w);
        lines.push(Line::from(vec![
            Span::styled("  ◆ ", Style::default().fg(dot_color)),
            Span::styled(label, Style::default().fg(theme.text)),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_deadlines_actions(
    frame: &mut Frame,
    area: Rect,
    deadlines: &[crate::memory_state::DeadlineItem],
    actions: &[crate::memory_state::ActionSummary],
    theme: &Theme,
) {
    let cols = Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(area);

    // ── Left: Deadlines ──
    if !deadlines.is_empty() {
        let mut lines: Vec<Line> = Vec::new();
        let max_w = cols[0].width.saturating_sub(4) as usize;
        lines.push(Line::from(Span::styled(
            " DEADLINES",
            Style::default().fg(theme.primary).add_modifier(Modifier::BOLD),
        )));
        for dl in deadlines.iter().take(4) {
            let color = deadline_color(dl.days_left, theme);
            let suffix = if dl.days_left >= 0 {
                format!(" ({}d)", dl.days_left)
            } else {
                " (overdue)".to_string()
            };
            let label = truncate(&format!("{}{}", dl.description, suffix), max_w);
            lines.push(Line::from(vec![
                Span::styled("  * ", Style::default().fg(color)),
                Span::styled(label, Style::default().fg(color)),
            ]));
        }
        frame.render_widget(Paragraph::new(lines), cols[0]);
    }

    // ── Right: Actions ──
    if !actions.is_empty() {
        let mut lines: Vec<Line> = Vec::new();
        let max_w = cols[1].width.saturating_sub(4) as usize;
        let count = actions.len();
        lines.push(Line::from(Span::styled(
            format!(" ACTIONS ({})", count),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        )));
        for act in actions.iter().take(4) {
            let label = truncate(&act.description, max_w);
            lines.push(Line::from(vec![
                Span::styled("  * ", Style::default().fg(theme.accent)),
                Span::styled(label, Style::default().fg(theme.accent)),
            ]));
        }
        frame.render_widget(Paragraph::new(lines), cols[1]);
    }
}

fn render_projects(
    frame: &mut Frame,
    area: Rect,
    projects: &[&crate::memory_state::ProjectSummary],
    theme: &Theme,
    width: u16,
) {
    let mut lines: Vec<Line> = Vec::new();
    let max_w = width.saturating_sub(4) as usize;
    lines.push(Line::from(Span::styled(
        " PROJECTS",
        Style::default().fg(theme.primary).add_modifier(Modifier::BOLD),
    )));
    for proj in projects.iter().take(4) {
        let blockers_str = if proj.blockers.is_empty() {
            String::new()
        } else {
            format!(" [blocked: {}]", proj.blockers.join(", "))
        };
        let deadline_str = proj.deadline.as_deref().map(|d| format!(" by {}", d)).unwrap_or_default();
        let detail = format!("{}{}{}", proj.status_line, deadline_str, blockers_str);
        let label = truncate(&format!("{}: {}", proj.name, detail), max_w);
        lines.push(Line::from(vec![
            Span::styled("  * ", Style::default().fg(theme.secondary)),
            Span::styled(label, Style::default().fg(theme.text)),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_decisions_waiting(
    frame: &mut Frame,
    area: Rect,
    decisions: &[crate::memory_state::DecisionSummary],
    waiting: &[crate::memory_state::WaitingItem],
    theme: &Theme,
) {
    let cols = Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(area);

    // ── Left: Decisions ──
    if !decisions.is_empty() {
        let mut lines: Vec<Line> = Vec::new();
        let max_w = cols[0].width.saturating_sub(4) as usize;
        lines.push(Line::from(Span::styled(
            " DECISIONS",
            Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
        )));
        for dec in decisions.iter().take(3) {
            let label = truncate(&dec.name, max_w);
            lines.push(Line::from(vec![
                Span::styled("  * ", Style::default().fg(theme.secondary)),
                Span::styled(label, Style::default().fg(theme.text)),
            ]));
        }
        frame.render_widget(Paragraph::new(lines), cols[0]);
    }

    // ── Right: Waiting On ──
    if !waiting.is_empty() {
        let mut lines: Vec<Line> = Vec::new();
        let max_w = cols[1].width.saturating_sub(4) as usize;
        lines.push(Line::from(Span::styled(
            " WAITING ON",
            Style::default().fg(theme.warning).add_modifier(Modifier::BOLD),
        )));
        for w in waiting.iter().take(3) {
            let suffix = if w.days_waiting > 0 {
                format!(" ({}d)", w.days_waiting)
            } else {
                String::new()
            };
            let label = truncate(&format!("{}{}", w.description, suffix), max_w);
            lines.push(Line::from(vec![
                Span::styled("  * ", Style::default().fg(theme.warning)),
                Span::styled(label, Style::default().fg(theme.warning)),
            ]));
        }
        frame.render_widget(Paragraph::new(lines), cols[1]);
    }
}

fn render_team_stale(
    frame: &mut Frame,
    area: Rect,
    team: &[String],
    stale: &[crate::memory_state::StaleItem],
    theme: &Theme,
) {
    let cols = Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(area);

    // ── Left: Team Pulse ──
    if !team.is_empty() {
        let mut lines: Vec<Line> = Vec::new();
        let max_w = cols[0].width.saturating_sub(4) as usize;
        lines.push(Line::from(Span::styled(
            " TEAM PULSE",
            Style::default().fg(theme.primary).add_modifier(Modifier::BOLD),
        )));
        for member in team.iter().take(3) {
            let label = truncate(member, max_w);
            lines.push(Line::from(vec![
                Span::styled("  * ", Style::default().fg(theme.primary)),
                Span::styled(label, Style::default().fg(theme.text)),
            ]));
        }
        frame.render_widget(Paragraph::new(lines), cols[0]);
    }

    // ── Right: Stale ──
    if !stale.is_empty() {
        let mut lines: Vec<Line> = Vec::new();
        let max_w = cols[1].width.saturating_sub(4) as usize;
        lines.push(Line::from(Span::styled(
            " STALE",
            Style::default().fg(theme.dimmed).add_modifier(Modifier::BOLD),
        )));
        for s in stale.iter().take(3) {
            let suffix = if s.days_stale > 0 {
                format!(" ({}d)", s.days_stale)
            } else {
                String::new()
            };
            let label = truncate(&format!("{}{}", s.entity_name, suffix), max_w);
            lines.push(Line::from(vec![
                Span::styled("  * ", Style::default().fg(theme.dimmed)),
                Span::styled(label, Style::default().fg(theme.dimmed)),
            ]));
        }
        frame.render_widget(Paragraph::new(lines), cols[1]);
    }
}

fn render_footer(
    frame: &mut Frame,
    area: Rect,
    ws: &MemoryWorldState,
    health: &Option<MemoryHealth>,
    theme: &Theme,
) {
    let staleness_dot = if ws.is_stale() {
        Span::styled("\u{25CF}", Style::default().fg(theme.warning))
    } else {
        Span::styled("\u{25CF}", Style::default().fg(theme.success))
    };

    let entity_count = health.as_ref().map(|h| h.entity_count).unwrap_or(ws.entity_count);
    let interaction_count = health.as_ref().map(|h| h.interaction_count).unwrap_or(ws.interaction_count);
    let freshness = if ws.is_stale() { "stale" } else { "fresh" };

    let mut spans = vec![
        Span::styled(" -- ", Style::default().fg(theme.dimmed)),
        Span::styled(
            format!("{} entities", entity_count),
            Style::default().fg(theme.dimmed),
        ),
        Span::styled(" | ", Style::default().fg(theme.dimmed)),
        Span::styled(
            format!("{} interactions", interaction_count),
            Style::default().fg(theme.dimmed),
        ),
        Span::styled(" | ", Style::default().fg(theme.dimmed)),
        Span::styled(format!("{} ", freshness), Style::default().fg(theme.dimmed)),
        staleness_dot,
        Span::styled("  --", Style::default().fg(theme.dimmed)),
    ];

    if let Some(h) = health {
        if !h.checked_at.is_empty() {
            spans.push(Span::styled(
                format!("  {}", h.checked_at),
                Style::default().fg(theme.dimmed),
            ));
        }
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_waiting_standalone(
    frame: &mut Frame,
    area: Rect,
    waiting: &[crate::memory_state::WaitingItem],
    theme: &Theme,
    width: u16,
) {
    let mut lines: Vec<Line> = Vec::new();
    let max_w = width.saturating_sub(4) as usize;
    let count = waiting.len();
    lines.push(Line::from(Span::styled(
        format!(" WAITING ON ({})", count),
        Style::default().fg(theme.warning).add_modifier(Modifier::BOLD),
    )));
    for w in waiting.iter().take(3) {
        let suffix = if w.days_waiting > 0 {
            format!(" ({}d)", w.days_waiting)
        } else {
            String::new()
        };
        let label = truncate(&format!("{}{}", w.description, suffix), max_w);
        lines.push(Line::from(vec![
            Span::styled("  \u{23f3} ", Style::default().fg(theme.warning)),
            Span::styled(label, Style::default().fg(theme.text)),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn deadline_color(days_left: i32, theme: &Theme) -> ratatui::style::Color {
    if days_left < 3 {
        theme.error
    } else if days_left < 7 {
        theme.warning
    } else {
        theme.success
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if max_len < 4 {
        return String::new();
    }
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let mut r: String = s.chars().take(max_len - 1).collect();
        r.push('\u{2026}');
        r
    }
}
