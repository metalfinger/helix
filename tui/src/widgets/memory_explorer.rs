use crate::memory_state::ExplorerData;
use crate::theme::Theme;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use std::time::Instant;

#[derive(Clone, Debug)]
pub enum ToastLevel {
    Success,
    Error,
    Info,
}

#[derive(Clone, Debug)]
pub struct ExplorerCmdResult {
    pub success: bool,
    pub message: String,
}

const TAB_NAMES: [&str; 5] = ["Projects", "People", "Decisions", "Timeline", "Actions"];

pub struct ExplorerState {
    pub active_tab: usize,
    pub tab_selected: [usize; 5],
    pub expanded: std::collections::HashSet<usize>,
    pub search_query: String,
    pub search_active: bool,
    pub detail_open: bool,
    pub detail_entity_id: String,
    pub detail_cursor: usize,
    pub detail_history: Vec<String>,
    // Command input mode (: prefix)
    pub command_active: bool,
    pub command_input: String,
    // Toast messages
    pub toast_message: String,
    pub toast_level: ToastLevel,
    pub toast_until: Option<Instant>,
    // Busy indicator (async operation in progress)
    pub busy: bool,
}

impl ExplorerState {
    pub fn new() -> Self {
        Self {
            active_tab: 0,
            tab_selected: [0; 5],
            expanded: std::collections::HashSet::new(),
            search_query: String::new(),
            search_active: false,
            detail_open: false,
            detail_entity_id: String::new(),
            detail_cursor: 0,
            detail_history: Vec::new(),
            command_active: false,
            command_input: String::new(),
            toast_message: String::new(),
            toast_level: ToastLevel::Info,
            toast_until: None,
            busy: false,
        }
    }

    pub fn reset(&mut self) {
        self.active_tab = 0;
        self.tab_selected = [0; 5];
        self.expanded.clear();
        self.search_query.clear();
        self.search_active = false;
        self.detail_open = false;
        self.detail_entity_id.clear();
        self.detail_cursor = 0;
        self.detail_history.clear();
        self.command_active = false;
        self.command_input.clear();
        self.toast_message.clear();
        self.toast_until = None;
        self.busy = false;
    }

    pub fn set_toast(&mut self, msg: String, level: ToastLevel) {
        self.toast_message = msg;
        self.toast_level = level;
        self.toast_until = Some(Instant::now() + std::time::Duration::from_secs(3));
        self.busy = false;
    }
}

pub fn render_memory_explorer(
    frame: &mut Frame,
    data: &ExplorerData,
    state: &ExplorerState,
    theme: &Theme,
) {
    let area = frame.area();

    // Clear background
    frame.render_widget(Clear, area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.dimmed))
        .style(Style::default().bg(Color::Black));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Layout: header(1) + tab_bar(1) + content(fill) + search(1 if active) + command(1 if active) + footer(1)
    let search_height = if state.search_active { 1 } else { 0 };
    let command_height = if state.command_active { 1 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Length(1), // tab bar
            Constraint::Min(0),   // content
            Constraint::Length(search_height), // search bar
            Constraint::Length(command_height), // command bar
            Constraint::Length(1), // footer
        ])
        .split(inner);

    render_header(frame, chunks[0], data, theme);
    render_tab_bar(frame, chunks[1], state.active_tab, data, theme);

    if state.detail_open {
        let content_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60),
                Constraint::Percentage(40),
            ])
            .split(chunks[2]);

        match state.active_tab {
            0 => render_projects_tab(frame, content_split[0], data, state, theme),
            1 => render_people_tab(frame, content_split[0], data, state, theme),
            2 => render_decisions_tab(frame, content_split[0], data, state, theme),
            3 => render_timeline_tab(frame, content_split[0], data, state, theme),
            4 => render_actions_tab(frame, content_split[0], data, state, theme),
            _ => {}
        }

        render_detail_pane(frame, content_split[1], data, state, theme);
    } else {
        match state.active_tab {
            0 => render_projects_tab(frame, chunks[2], data, state, theme),
            1 => render_people_tab(frame, chunks[2], data, state, theme),
            2 => render_decisions_tab(frame, chunks[2], data, state, theme),
            3 => render_timeline_tab(frame, chunks[2], data, state, theme),
            4 => render_actions_tab(frame, chunks[2], data, state, theme),
            _ => {}
        }
    }

    if state.search_active {
        render_search_bar(frame, chunks[3], state, theme);
    }
    if state.command_active {
        render_command_bar(frame, chunks[4], state, theme);
    }
    render_footer(frame, chunks[5], state, data, theme);
}

fn render_header(frame: &mut Frame, area: ratatui::layout::Rect, data: &ExplorerData, theme: &Theme) {
    // Count overdue actions
    let overdue_count = data.actions.iter().filter(|a| {
        a.deadline.as_ref().map_or(false, |d| {
            parse_days_left(d).map_or(false, |dl| dl < 0)
        })
    }).count();
    // Count actions due within 3 days
    let urgent_count = data.actions.iter().filter(|a| {
        a.deadline.as_ref().map_or(false, |d| {
            parse_days_left(d).map_or(false, |dl| dl >= 0 && dl <= 3)
        })
    }).count();
    // Count blocked projects
    let blocked_count = data.projects.iter().filter(|p| !p.blockers.is_empty()).count();

    let mut urgency_parts: Vec<String> = Vec::new();
    if overdue_count > 0 {
        urgency_parts.push(format!("{} overdue", overdue_count));
    }
    if urgent_count > 0 {
        urgency_parts.push(format!("{} due soon", urgent_count));
    }
    if blocked_count > 0 {
        urgency_parts.push(format!("{} blocked", blocked_count));
    }

    let urgency_str = if urgency_parts.is_empty() {
        String::new()
    } else {
        format!(" \u{26A0} {}", urgency_parts.join(", "))
    };

    let spans = vec![
        Span::styled(
            format!(
                " MEMORY EXPLORER \u{2014} {} projects, {} people, {} decisions, {} actions",
                data.projects.len(),
                data.people.len(),
                data.decisions.len(),
                data.actions.len(),
            ),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            urgency_str,
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
    ];
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_tab_bar(frame: &mut Frame, area: ratatui::layout::Rect, active: usize, data: &ExplorerData, theme: &Theme) {
    let counts = [
        data.projects.len(),
        data.people.len(),
        data.decisions.len(),
        data.timeline.len(),
        data.actions.len(),
    ];
    let mut spans: Vec<Span> = vec![Span::styled(" ", Style::default())];
    for (i, name) in TAB_NAMES.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" \u{2502} ", Style::default().fg(theme.dimmed)));
        }
        let label = format!(" {} ({}) ", name, counts[i]);
        if i == active {
            spans.push(Span::styled(
                label,
                Style::default().fg(Color::Black).bg(theme.accent).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                label,
                Style::default().fg(theme.text),
            ));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn matches_search(text: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let lower = text.to_lowercase();
    let q = query.to_lowercase();
    lower.contains(&q)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else if max <= 1 {
        "\u{2026}".to_string()
    } else {
        let mut r: String = s.chars().take(max - 1).collect();
        r.push('\u{2026}');
        r
    }
}

fn format_date_short(d: &Option<String>) -> String {
    match d {
        Some(s) if !s.is_empty() => {
            // Take first 10 chars (YYYY-MM-DD)
            s.chars().take(10).collect()
        }
        _ => "\u{2014}".to_string(),
    }
}

fn parse_days_left(deadline: &str) -> Option<i64> {
    // Parse YYYY-MM-DD and compute days from today
    let parts: Vec<&str> = deadline.split('-').collect();
    if parts.len() != 3 { return None; }
    let year: i32 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;
    let deadline_date = chrono::NaiveDate::from_ymd_opt(year, month, day)?;
    let today = chrono::Local::now().date_naive();
    Some((deadline_date - today).num_days())
}

fn scope_icon(scope: &str) -> &str {
    match scope {
        "aeos" => "\u{26A1}",
        "metalfinger" => "\u{1F918}",
        "personal" => "\u{1F3E0}",
        _ => "\u{1F310}",
    }
}

fn source_color(source: &str) -> Color {
    match source {
        "email" => Color::Rgb(100, 149, 237),
        "mattermost" => Color::Rgb(35, 137, 218),
        "telegram" => Color::Rgb(0, 136, 204),
        "claude_code" => Color::Rgb(204, 120, 50),
        "plane" => Color::Rgb(60, 179, 113),
        "meeting" | "call" | "in_person" => Color::Rgb(255, 165, 0),
        "manual" => Color::Rgb(169, 169, 169),
        _ => Color::DarkGray,
    }
}

fn status_color(status: &str) -> Color {
    match status {
        "pending" => Color::Yellow,
        "in_progress" => Color::Cyan,
        "done" => Color::Green,
        "cancelled" => Color::DarkGray,
        "stale" => Color::Red,
        _ => Color::Gray,
    }
}

/// Unified entity info extracted from any explorer card type.
struct DetailEntity {
    name: String,
    entity_type: String,
    scope: String,
    context: String,
    status: String,
    deadline: Option<String>,
    links_to: Vec<(String, String)>,
}

/// Relation link displayed in the detail pane.
struct RelationLink {
    direction: &'static str,
    relation_type: String,
    entity_name: String,
}

fn find_entity_by_name(data: &ExplorerData, name_or_id: &str) -> Option<DetailEntity> {
    for p in &data.projects {
        if p.entity_id == name_or_id || p.name == name_or_id {
            let mut links_to: Vec<(String, String)> = Vec::new();
            for person in &p.key_people {
                links_to.push(("team_member".to_string(), person.clone()));
            }
            for blocker in &p.blockers {
                links_to.push(("blocked_by".to_string(), blocker.clone()));
            }
            return Some(DetailEntity {
                name: p.name.clone(),
                entity_type: "project".to_string(),
                scope: p.scope.clone(),
                context: p.context.clone(),
                status: p.status.clone(),
                deadline: p.deadline.clone(),
                links_to,
            });
        }
    }
    for p in &data.people {
        if p.entity_id == name_or_id || p.name == name_or_id {
            let links_to: Vec<(String, String)> = p.projects.iter()
                .map(|proj| ("works_on".to_string(), proj.clone()))
                .collect();
            return Some(DetailEntity {
                name: p.name.clone(),
                entity_type: p.role.clone(),
                scope: p.scope.clone(),
                context: if p.context.is_empty() {
                    p.projects.iter().map(|proj| format!("Works on: {}", proj)).collect::<Vec<_>>().join(", ")
                } else {
                    p.context.clone()
                },
                status: String::new(),
                deadline: p.last_contact.clone(),
                links_to,
            });
        }
    }
    for d in &data.decisions {
        if d.entity_id == name_or_id || d.name == name_or_id {
            let links_to: Vec<(String, String)> = d.related_entities.iter()
                .map(|e| ("related_to".to_string(), e.clone()))
                .collect();
            return Some(DetailEntity {
                name: d.name.clone(),
                entity_type: "decision".to_string(),
                scope: String::new(),
                context: d.summary.clone(),
                status: String::new(),
                deadline: d.date.clone(),
                links_to,
            });
        }
    }
    for a in &data.actions {
        if a.id == name_or_id || a.description == name_or_id {
            let mut links_to: Vec<(String, String)> = Vec::new();
            if !a.project.is_empty() {
                links_to.push(("belongs_to".to_string(), a.project.clone()));
            }
            return Some(DetailEntity {
                name: a.description.clone(),
                entity_type: "task".to_string(),
                scope: String::new(),
                context: String::new(),
                status: a.status.clone(),
                deadline: a.deadline.clone(),
                links_to,
            });
        }
    }
    None
}

fn find_backlinks(data: &ExplorerData, entity_name: &str) -> Vec<RelationLink> {
    let mut links: Vec<RelationLink> = Vec::new();
    for p in &data.projects {
        if p.key_people.iter().any(|n| n == entity_name) {
            links.push(RelationLink {
                direction: "\u{2190}",
                relation_type: "team_member".to_string(),
                entity_name: p.name.clone(),
            });
        }
        if p.blockers.iter().any(|n| n == entity_name) {
            links.push(RelationLink {
                direction: "\u{2190}",
                relation_type: "blocked_by".to_string(),
                entity_name: p.name.clone(),
            });
        }
    }
    for p in &data.people {
        if p.projects.iter().any(|n| n == entity_name) {
            links.push(RelationLink {
                direction: "\u{2190}",
                relation_type: "works_on".to_string(),
                entity_name: p.name.clone(),
            });
        }
    }
    for d in &data.decisions {
        if d.related_entities.iter().any(|n| n == entity_name) {
            links.push(RelationLink {
                direction: "\u{2190}",
                relation_type: "related_to".to_string(),
                entity_name: d.name.clone(),
            });
        }
    }
    for a in &data.actions {
        if a.project == entity_name {
            links.push(RelationLink {
                direction: "\u{2190}",
                relation_type: "belongs_to".to_string(),
                entity_name: a.description.clone(),
            });
        }
    }
    links
}

fn get_linked_names_for_card(data: &ExplorerData, tab: usize, card_idx: usize, search_query: &str) -> Vec<String> {
    match tab {
        0 => {
            let mut visible_idx = 0;
            for proj in &data.projects {
                if !search_query.is_empty()
                    && !matches_search(&proj.name, search_query)
                    && !matches_search(&proj.context, search_query)
                    && !matches_search(&proj.scope, search_query)
                {
                    continue;
                }
                if visible_idx == card_idx {
                    let mut names: Vec<String> = Vec::new();
                    names.extend(proj.key_people.iter().cloned());
                    names.extend(proj.blockers.iter().cloned());
                    return names;
                }
                visible_idx += 1;
            }
            Vec::new()
        }
        1 => {
            let mut visible_idx = 0;
            for person in &data.people {
                if !search_query.is_empty()
                    && !matches_search(&person.name, search_query)
                    && !matches_search(&person.role, search_query)
                {
                    continue;
                }
                if visible_idx == card_idx {
                    return person.projects.clone();
                }
                visible_idx += 1;
            }
            Vec::new()
        }
        2 => {
            let mut visible_idx = 0;
            for dec in &data.decisions {
                if !search_query.is_empty()
                    && !matches_search(&dec.name, search_query)
                    && !matches_search(&dec.summary, search_query)
                {
                    continue;
                }
                if visible_idx == card_idx {
                    return dec.related_entities.clone();
                }
                visible_idx += 1;
            }
            Vec::new()
        }
        3 => {
            let mut visible_idx = 0;
            for entry in &data.timeline {
                if !search_query.is_empty()
                    && !matches_search(&entry.summary, search_query)
                    && !matches_search(&entry.source, search_query)
                    && !entry.entity_names.iter().any(|n| matches_search(n, search_query))
                {
                    continue;
                }
                if visible_idx == card_idx {
                    return entry.entity_names.clone();
                }
                visible_idx += 1;
            }
            Vec::new()
        }
        4 => {
            let mut visible_idx = 0;
            for action in &data.actions {
                if !search_query.is_empty()
                    && !matches_search(&action.description, search_query)
                    && !matches_search(&action.project, search_query)
                {
                    continue;
                }
                if visible_idx == card_idx {
                    if action.project.is_empty() { return Vec::new(); }
                    return vec![action.project.clone()];
                }
                visible_idx += 1;
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

// Public wrappers for app.rs
pub fn find_entity_by_name_pub(data: &ExplorerData, name: &str) -> Option<(String, Vec<(String, String)>)> {
    find_entity_by_name(data, name).map(|e| (e.name, e.links_to))
}

pub fn find_backlinks_pub(data: &ExplorerData, entity_name: &str) -> Vec<(String, String)> {
    find_backlinks(data, entity_name).iter()
        .map(|l| (l.relation_type.clone(), l.entity_name.clone()))
        .collect()
}

pub fn get_linked_names_pub(data: &ExplorerData, tab: usize, card_idx: usize, search_query: &str) -> Vec<String> {
    get_linked_names_for_card(data, tab, card_idx, search_query)
}

/// Get the name/id and type of the currently selected entity in a tab.
/// Returns (display_name, entity_type, current_status_or_priority).
/// IMPORTANT: Applies the SAME sort/group order as the render functions so
/// the index matches what the user sees on screen.
pub fn get_selected_entity_info(
    data: &ExplorerData,
    tab: usize,
    selected: usize,
    search_query: &str,
) -> Option<(String, String, String)> {
    match tab {
        0 => {
            // Must match render_projects_tab: filter → group by scope → sort by project_sort_key
            let filtered: Vec<&crate::memory_state::ExplorerProjectCard> = data.projects.iter()
                .filter(|p| search_query.is_empty() || matches_search(&p.name, search_query) || matches_search(&p.context, search_query) || matches_search(&p.scope, search_query))
                .collect();
            let scope_order = ["aeos", "metalfinger", "personal", "global"];
            let mut grouped: std::collections::BTreeMap<String, Vec<&crate::memory_state::ExplorerProjectCard>> =
                std::collections::BTreeMap::new();
            let mut project_order: Vec<String> = Vec::new();
            for proj in &filtered {
                let scope = if proj.scope.is_empty() { "other".to_string() } else { proj.scope.clone() };
                if !project_order.contains(&scope) { project_order.push(scope.clone()); }
                grouped.entry(scope).or_default().push(proj);
            }
            for projects in grouped.values_mut() {
                projects.sort_by(|a, b| project_sort_key(a).cmp(&project_sort_key(b)));
            }
            let mut ordered_scopes: Vec<String> = Vec::new();
            for s in &scope_order { if grouped.contains_key(*s) { ordered_scopes.push(s.to_string()); } }
            for s in grouped.keys() { if !ordered_scopes.contains(s) { ordered_scopes.push(s.clone()); } }
            let mut flat: Vec<&crate::memory_state::ExplorerProjectCard> = Vec::new();
            for scope in &ordered_scopes {
                if let Some(projs) = grouped.get(scope) { flat.extend(projs); }
            }
            flat.get(selected).map(|p| (p.name.clone(), "project".to_string(), p.status.clone()))
        }
        1 => {
            // Must match render_people_tab: filter → group by scope → sort by last_contact desc
            let filtered: Vec<&crate::memory_state::ExplorerPersonCard> = data.people.iter()
                .filter(|p| search_query.is_empty() || matches_search(&p.name, search_query) || matches_search(&p.role, search_query))
                .collect();
            let scope_order = ["aeos", "metalfinger", "personal", "global"];
            let mut grouped: std::collections::BTreeMap<String, Vec<&crate::memory_state::ExplorerPersonCard>> =
                std::collections::BTreeMap::new();
            for person in &filtered {
                let scope = if person.scope.is_empty() { "other".to_string() } else { person.scope.clone() };
                grouped.entry(scope).or_default().push(person);
            }
            for people in grouped.values_mut() {
                people.sort_by(|a, b| b.last_contact.cmp(&a.last_contact));
            }
            let mut ordered_scopes: Vec<String> = Vec::new();
            for s in &scope_order { if grouped.contains_key(*s) { ordered_scopes.push(s.to_string()); } }
            for s in grouped.keys() { if !ordered_scopes.contains(s) { ordered_scopes.push(s.clone()); } }
            let mut flat: Vec<&crate::memory_state::ExplorerPersonCard> = Vec::new();
            for scope in &ordered_scopes {
                if let Some(people) = grouped.get(scope) { flat.extend(people); }
            }
            flat.get(selected).map(|p| (p.name.clone(), "person".to_string(), String::new()))
        }
        2 => {
            // Must match render_decisions_tab: filter → sort by date desc
            let mut filtered: Vec<&crate::memory_state::ExplorerDecisionCard> = data.decisions.iter()
                .filter(|d| search_query.is_empty() || matches_search(&d.name, search_query) || matches_search(&d.summary, search_query))
                .collect();
            filtered.sort_by(|a, b| match (&b.date, &a.date) {
                (Some(bd), Some(ad)) => bd.cmp(ad),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            });
            filtered.get(selected).map(|d| (d.name.clone(), "decision".to_string(), String::new()))
        }
        3 => {
            // Timeline: filter only (render doesn't reorder entries, just groups by date)
            let filtered: Vec<_> = data.timeline.iter()
                .filter(|e| search_query.is_empty() || matches_search(&e.summary, search_query) || matches_search(&e.source, search_query)
                    || e.entity_names.iter().any(|n| matches_search(n, search_query)))
                .collect();
            filtered.get(selected).map(|e| (e.summary.clone(), "timeline".to_string(), String::new()))
        }
        4 => {
            // Must match render_actions_tab: filter → sort by action_sort_key → group by project
            let mut filtered: Vec<&crate::memory_state::ExplorerActionCard> = data.actions.iter()
                .filter(|a| search_query.is_empty() || matches_search(&a.description, search_query) || matches_search(&a.project, search_query))
                .collect();
            filtered.sort_by(|a, b| action_sort_key(a).cmp(&action_sort_key(b)));
            // Group by project preserving sort order (same as render)
            let mut project_order: Vec<String> = Vec::new();
            let mut grouped: std::collections::BTreeMap<String, Vec<&crate::memory_state::ExplorerActionCard>> =
                std::collections::BTreeMap::new();
            for action in &filtered {
                let proj = if action.project.is_empty() { "Unlinked".to_string() } else { action.project.clone() };
                if !project_order.contains(&proj) { project_order.push(proj.clone()); }
                grouped.entry(proj).or_default().push(action);
            }
            let mut flat: Vec<&crate::memory_state::ExplorerActionCard> = Vec::new();
            for proj in &project_order {
                if let Some(actions) = grouped.get(proj) { flat.extend(actions); }
            }
            flat.get(selected).map(|a| (a.description.clone(), "task".to_string(), a.status.clone()))
        }
        _ => None,
    }
}

/// Sort key for projects: active first, then paused, then completed/archived.
/// Within same status, projects with sooner deadlines come first.
fn project_sort_key(proj: &crate::memory_state::ExplorerProjectCard) -> (u8, i64) {
    let status_bucket = match proj.status.as_str() {
        "active" => 0,
        "paused" => 1,
        "completed" => 2,
        "archived" => 3,
        _ => 4,
    };
    let deadline_key = match &proj.deadline {
        Some(d) if !d.is_empty() => parse_days_left(d).unwrap_or(9999),
        _ => 9999,
    };
    (status_bucket, deadline_key)
}

fn render_projects_tab(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    data: &ExplorerData,
    state: &ExplorerState,
    theme: &Theme,
) {
    let selected = state.tab_selected[0];
    let mut lines: Vec<Line> = Vec::new();
    let mut item_idx: usize = 0;
    let mut item_line_starts: Vec<usize> = Vec::new();

    // Filter
    let filtered: Vec<&crate::memory_state::ExplorerProjectCard> = data.projects.iter()
        .filter(|p| {
            state.search_query.is_empty()
                || matches_search(&p.name, &state.search_query)
                || matches_search(&p.context, &state.search_query)
                || matches_search(&p.scope, &state.search_query)
        })
        .collect();

    // Group by scope
    let scope_order = ["aeos", "metalfinger", "personal", "global"];
    let mut grouped: std::collections::BTreeMap<String, Vec<&crate::memory_state::ExplorerProjectCard>> =
        std::collections::BTreeMap::new();
    for proj in &filtered {
        let scope = if proj.scope.is_empty() { "other".to_string() } else { proj.scope.clone() };
        grouped.entry(scope).or_default().push(proj);
    }

    // Sort within each scope: active first, then by deadline
    for projects in grouped.values_mut() {
        projects.sort_by(|a, b| project_sort_key(a).cmp(&project_sort_key(b)));
    }

    // Render in scope order
    let mut rendered_scopes: Vec<String> = Vec::new();
    for scope in &scope_order {
        if grouped.contains_key(*scope) {
            rendered_scopes.push(scope.to_string());
        }
    }
    for scope in grouped.keys() {
        if !rendered_scopes.contains(scope) {
            rendered_scopes.push(scope.clone());
        }
    }

    for scope in &rendered_scopes {
        let projects = match grouped.get(scope) {
            Some(p) => p,
            None => continue,
        };

        // Scope header
        let active_count = projects.iter().filter(|p| p.status == "active").count();
        let count_detail = if active_count == projects.len() {
            format!("({})", projects.len())
        } else {
            format!("({} active, {} total)", active_count, projects.len())
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} {} ", scope_icon(scope), scope),
                Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
            ),
            Span::styled(count_detail, Style::default().fg(Color::DarkGray)),
        ]));

        for proj in projects {
            item_line_starts.push(lines.len());
            let is_selected = item_idx == selected;
            let is_expanded = state.expanded.contains(&item_idx);
            let marker = if is_selected { "\u{25B8} " } else { "  " };

            let base_style = if is_selected {
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };

            // Status badge
            let status_str = match proj.status.as_str() {
                "active" => "\u{25CF}",
                "paused" => "\u{25D0}",
                "completed" => "\u{2713}",
                _ => "\u{25CB}",
            };
            let status_c = match proj.status.as_str() {
                "active" => Color::Green,
                "paused" => Color::Yellow,
                "completed" => Color::DarkGray,
                _ => Color::Gray,
            };

            // Deadline with days-left computation
            let deadline_str = match &proj.deadline {
                Some(d) if !d.is_empty() => {
                    if let Some(days_left) = parse_days_left(d) {
                        if days_left < 0 {
                            (format!(" {}d overdue", -days_left), Color::Red)
                        } else if days_left == 0 {
                            (" TODAY".to_string(), Color::Red)
                        } else if days_left <= 7 {
                            (format!(" {}d left", days_left), Color::Yellow)
                        } else {
                            (format!(" {}d left", days_left), Color::DarkGray)
                        }
                    } else {
                        (format!(" due:{}", d), Color::Yellow)
                    }
                }
                _ => (String::new(), Color::DarkGray),
            };

            let mut spans = vec![
                Span::styled(marker.to_string(), base_style),
                Span::styled(status_str, Style::default().fg(status_c)),
                Span::styled(format!(" {}", proj.name), base_style),
            ];
            if !deadline_str.0.is_empty() {
                spans.push(Span::styled(deadline_str.0, Style::default().fg(deadline_str.1)));
            }
            if !proj.blockers.is_empty() {
                spans.push(Span::styled(
                    format!(" \u{26D4}{}", proj.blockers.len()),
                    Style::default().fg(Color::Red),
                ));
            }
            lines.push(Line::from(spans));

            if is_expanded {
                // Context
                if !proj.context.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("    {}", truncate(&proj.context, area.width.saturating_sub(6) as usize)),
                        Style::default().fg(Color::Gray),
                    )));
                }
                // Team
                if !proj.key_people.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("    Team: {}", proj.key_people.join(", ")),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                // Blockers
                if !proj.blockers.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("    Blockers: {}", proj.blockers.join(", ")),
                        Style::default().fg(Color::Red),
                    )));
                }
                // Pending actions
                if !proj.pending_actions.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "    Action Items:",
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    )));
                }
                for a in &proj.pending_actions {
                    lines.push(Line::from(vec![
                        Span::styled("    \u{25CB} ", Style::default().fg(Color::Yellow)),
                        Span::styled(truncate(a, area.width.saturating_sub(8) as usize), Style::default().fg(Color::Gray)),
                    ]));
                }
                // Tasks
                if !proj.tasks.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "    Tasks:",
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    )));
                    for t in &proj.tasks {
                        lines.push(Line::from(vec![
                            Span::styled("    ", Style::default()),
                            Span::styled("\u{25C6} ", Style::default().fg(Color::Cyan)),
                            Span::styled(truncate(t, area.width.saturating_sub(8) as usize), Style::default().fg(Color::Gray)),
                        ]));
                    }
                }
                // Recent interactions (last 3)
                for inter in proj.recent_interactions.iter().take(3) {
                    let ts = format_date_short(&Some(inter.timestamp.clone()));
                    lines.push(Line::from(vec![
                        Span::styled("    ", Style::default()),
                        Span::styled(format!("[{}]", inter.source), Style::default().fg(source_color(&inter.source))),
                        Span::styled(format!(" {} \u{2014} {}", ts, truncate(&inter.summary, area.width.saturating_sub(24) as usize)), Style::default().fg(Color::DarkGray)),
                    ]));
                }
                lines.push(Line::from(""));
            }

            item_idx += 1;
        }

        lines.push(Line::from(""));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No projects found. Use helix_create_entity(type=\"project\") to add one.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let selected_line = item_line_starts.get(selected).copied().unwrap_or(0);
    let half_height = area.height as usize / 2;
    let scroll = if selected_line > half_height {
        (selected_line - half_height) as u16
    } else {
        0
    };

    let content = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(content, area);
}

fn render_people_tab(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    data: &ExplorerData,
    state: &ExplorerState,
    theme: &Theme,
) {
    let selected = state.tab_selected[1];
    let mut lines: Vec<Line> = Vec::new();
    let mut item_idx: usize = 0;
    let mut item_line_starts: Vec<usize> = Vec::new();

    // Filter by search
    let filtered: Vec<&crate::memory_state::ExplorerPersonCard> = data.people.iter()
        .filter(|p| {
            state.search_query.is_empty()
                || matches_search(&p.name, &state.search_query)
                || matches_search(&p.role, &state.search_query)
        })
        .collect();

    // Group by scope, ordered: aeos → metalfinger → personal → global → other
    let scope_order = ["aeos", "metalfinger", "personal", "global"];
    let mut grouped: std::collections::BTreeMap<String, Vec<&crate::memory_state::ExplorerPersonCard>> =
        std::collections::BTreeMap::new();
    for person in &filtered {
        let scope = if person.scope.is_empty() { "other".to_string() } else { person.scope.clone() };
        grouped.entry(scope).or_default().push(person);
    }

    // Sort people within each scope by last_contact (most recent first)
    for people in grouped.values_mut() {
        people.sort_by(|a, b| b.last_contact.cmp(&a.last_contact));
    }

    // Render in scope order, then any remaining scopes
    let mut rendered_scopes: Vec<String> = Vec::new();
    for scope in &scope_order {
        if grouped.contains_key(*scope) {
            rendered_scopes.push(scope.to_string());
        }
    }
    for scope in grouped.keys() {
        if !rendered_scopes.contains(scope) {
            rendered_scopes.push(scope.clone());
        }
    }

    for scope in &rendered_scopes {
        let people = match grouped.get(scope) {
            Some(p) => p,
            None => continue,
        };

        // Scope header
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} {} ", scope_icon(scope), scope),
                Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("({})", people.len()),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        for person in people {
            item_line_starts.push(lines.len());
            let is_selected = item_idx == selected;
            let is_expanded = state.expanded.contains(&item_idx);
            let marker = if is_selected { "\u{25B8} " } else { "  " };

            let base_style = if is_selected {
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };

            let last_contact_str = match &person.last_contact {
                Some(d) => format!("  last: {}", d),
                None => String::new(),
            };

            lines.push(Line::from(vec![
                Span::styled(marker.to_string(), base_style),
                Span::styled(person.name.clone(), base_style),
                Span::styled(format!(" ({})", person.role), Style::default().fg(Color::DarkGray)),
                Span::styled(last_contact_str, Style::default().fg(Color::DarkGray)),
            ]));

            if is_expanded {
                if !person.projects.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("    Projects: {}", person.projects.join(", ")),
                        Style::default().fg(Color::Gray),
                    )));
                }
                for inter in person.recent_interactions.iter().take(3) {
                    let ts = format_date_short(&Some(inter.timestamp.clone()));
                    lines.push(Line::from(vec![
                        Span::styled("    ", Style::default()),
                        Span::styled(format!("[{}]", inter.source), Style::default().fg(source_color(&inter.source))),
                        Span::styled(format!(" {} \u{2014} {}", ts, truncate(&inter.summary, area.width.saturating_sub(24) as usize)), Style::default().fg(Color::DarkGray)),
                    ]));
                }
                lines.push(Line::from(""));
            }

            item_idx += 1;
        }

        lines.push(Line::from(""));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No people found. Use helix_create_entity(type=\"person\") to add contacts.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let selected_line = item_line_starts.get(selected).copied().unwrap_or(0);
    let half_height = area.height as usize / 2;
    let scroll = if selected_line > half_height {
        (selected_line - half_height) as u16
    } else {
        0
    };
    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), area);
}

fn render_decisions_tab(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    data: &ExplorerData,
    state: &ExplorerState,
    theme: &Theme,
) {
    let selected = state.tab_selected[2];
    let mut lines: Vec<Line> = Vec::new();
    let mut item_idx: usize = 0;
    let mut item_line_starts: Vec<usize> = Vec::new();

    // Filter and sort decisions by date (most recent first, no-date at bottom)
    let mut filtered: Vec<&crate::memory_state::ExplorerDecisionCard> = data.decisions.iter()
        .filter(|d| {
            state.search_query.is_empty()
                || matches_search(&d.name, &state.search_query)
                || matches_search(&d.summary, &state.search_query)
        })
        .collect();

    filtered.sort_by(|a, b| {
        // Sort by date descending (most recent first), None last
        match (&b.date, &a.date) {
            (Some(bd), Some(ad)) => bd.cmp(ad),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    for dec in &filtered {
        item_line_starts.push(lines.len());
        let is_selected = item_idx == selected;
        let is_expanded = state.expanded.contains(&item_idx);
        let marker = if is_selected { "\u{25B8} " } else { "  " };
        let base_style = if is_selected {
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };

        let date_str = format_date_short(&dec.date);

        lines.push(Line::from(vec![
            Span::styled(marker.to_string(), base_style),
            Span::styled(format!("{} ", date_str), Style::default().fg(Color::DarkGray)),
            Span::styled(dec.name.clone(), base_style),
        ]));

        if is_expanded {
            // Full summary
            if !dec.summary.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("    {}", truncate(&dec.summary, area.width.saturating_sub(6) as usize)),
                    Style::default().fg(Color::Gray),
                )));
            }
            // Related entities as navigable links
            if !dec.related_entities.is_empty() {
                lines.push(Line::from(Span::styled(
                    "    Related:",
                    Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
                )));
                for re in &dec.related_entities {
                    lines.push(Line::from(vec![
                        Span::styled("      \u{2192} ", Style::default().fg(theme.secondary)),
                        Span::styled(re.clone(), Style::default().fg(theme.text)),
                    ]));
                }
            }
            lines.push(Line::from(""));
        }

        item_idx += 1;
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No decisions found. Use helix_remember(type=\"decision\") to log one.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let selected_line = item_line_starts.get(selected).copied().unwrap_or(0);
    let half_height = area.height as usize / 2;
    let scroll = if selected_line > half_height {
        (selected_line - half_height) as u16
    } else {
        0
    };
    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), area);
}

/// Classify a timestamp into a human-readable date group.
fn date_group_label(timestamp: &str) -> String {
    let date_str: String = timestamp.chars().take(10).collect();
    let today = chrono::Local::now().date_naive();
    if let Some(entry_date) = {
        let parts: Vec<&str> = date_str.split('-').collect();
        if parts.len() == 3 {
            let y: i32 = parts[0].parse().unwrap_or(0);
            let m: u32 = parts[1].parse().unwrap_or(0);
            let d: u32 = parts[2].parse().unwrap_or(0);
            chrono::NaiveDate::from_ymd_opt(y, m, d)
        } else {
            None
        }
    } {
        let diff = (today - entry_date).num_days();
        if diff == 0 {
            "Today".to_string()
        } else if diff == 1 {
            "Yesterday".to_string()
        } else if diff < 7 {
            format!("This Week ({}d ago)", diff)
        } else if diff < 30 {
            format!("This Month ({}d ago)", diff)
        } else {
            "Older".to_string()
        }
    } else {
        "Unknown Date".to_string()
    }
}

fn render_timeline_tab(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    data: &ExplorerData,
    state: &ExplorerState,
    theme: &Theme,
) {
    let selected = state.tab_selected[3];
    let mut lines: Vec<Line> = Vec::new();
    let mut item_idx: usize = 0;
    let mut item_line_starts: Vec<usize> = Vec::new();

    // Filter
    let filtered: Vec<&crate::memory_state::ExplorerTimelineEntry> = data.timeline.iter()
        .filter(|e| {
            state.search_query.is_empty()
                || matches_search(&e.summary, &state.search_query)
                || matches_search(&e.source, &state.search_query)
                || e.entity_names.iter().any(|n| matches_search(n, &state.search_query))
        })
        .collect();

    // Group by date
    let mut current_group = String::new();

    for entry in &filtered {
        let group = date_group_label(&entry.timestamp);
        if group != current_group {
            if !current_group.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                format!(" \u{2500} {} ", group),
                Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
            )));
            current_group = group;
        }

        item_line_starts.push(lines.len());
        let is_selected = item_idx == selected;
        let is_expanded = state.expanded.contains(&item_idx);
        let marker = if is_selected { "\u{25B8} " } else { "  " };
        let base_style = if is_selected {
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text)
        };

        // Show time portion if available, otherwise date
        let ts: String = if entry.timestamp.len() >= 16 {
            entry.timestamp.chars().skip(11).take(5).collect() // HH:MM
        } else {
            format_date_short(&Some(entry.timestamp.clone()))
        };

        lines.push(Line::from(vec![
            Span::styled(marker.to_string(), base_style),
            Span::styled(format!("{} ", ts), Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:<12}", entry.source), Style::default().fg(source_color(&entry.source))),
            Span::styled(
                truncate(&entry.summary, area.width.saturating_sub(30) as usize),
                base_style,
            ),
        ]));

        if is_expanded {
            lines.push(Line::from(Span::styled(
                format!("    {}", entry.summary),
                Style::default().fg(Color::Gray),
            )));
            if !entry.r#type.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("    Type: {}", entry.r#type),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            if !entry.entity_names.is_empty() {
                lines.push(Line::from(Span::styled(
                    "    Entities:",
                    Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
                )));
                for en in &entry.entity_names {
                    lines.push(Line::from(vec![
                        Span::styled("      \u{2192} ", Style::default().fg(theme.secondary)),
                        Span::styled(en.clone(), Style::default().fg(theme.text)),
                    ]));
                }
            }
            lines.push(Line::from(""));
        } else if !entry.entity_names.is_empty() {
            lines.last_mut().map(|l| {
                l.spans.push(Span::styled(
                    format!(" [{}]", entry.entity_names.join(", ")),
                    Style::default().fg(Color::DarkGray),
                ));
            });
        }

        item_idx += 1;
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No timeline entries. Use helix_remember or helix_log_interaction to add context.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let selected_line = item_line_starts.get(selected).copied().unwrap_or(0);
    let half_height = area.height as usize / 2;
    let scroll = if selected_line > half_height {
        (selected_line - half_height) as u16
    } else {
        0
    };
    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), area);
}

/// Compute a sort key for an action: actions with deadlines sort by days_left
/// (overdue first, then soonest), actions without deadlines sort to the bottom by age.
fn action_sort_key(action: &crate::memory_state::ExplorerActionCard) -> (u8, i64) {
    match &action.deadline {
        Some(d) if !d.is_empty() => {
            if let Some(days_left) = parse_days_left(d) {
                (0, days_left) // bucket 0 = has deadline, sort by days_left ascending
            } else {
                (1, 0) // unparseable deadline — after valid deadlines
            }
        }
        _ => (2, -(action.age_days as i64)), // bucket 2 = no deadline, oldest first
    }
}

fn render_actions_tab(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    data: &ExplorerData,
    state: &ExplorerState,
    theme: &Theme,
) {
    let selected = state.tab_selected[4];
    let mut lines: Vec<Line> = Vec::new();
    let mut item_idx: usize = 0;
    let mut item_line_starts: Vec<usize> = Vec::new();

    // Filter actions by search query
    let mut filtered: Vec<&crate::memory_state::ExplorerActionCard> = data.actions.iter()
        .filter(|a| {
            state.search_query.is_empty()
                || matches_search(&a.description, &state.search_query)
                || matches_search(&a.project, &state.search_query)
        })
        .collect();

    // Sort by deadline: fewest days left first, no-deadline at bottom
    filtered.sort_by(|a, b| action_sort_key(a).cmp(&action_sort_key(b)));

    // Group by project (preserve sort order within each group)
    let mut grouped: std::collections::BTreeMap<String, Vec<&crate::memory_state::ExplorerActionCard>> =
        std::collections::BTreeMap::new();
    // Track insertion order so projects with most urgent actions appear first
    let mut project_order: Vec<String> = Vec::new();
    for action in &filtered {
        let proj = if action.project.is_empty() {
            "Unlinked".to_string()
        } else {
            action.project.clone()
        };
        if !project_order.contains(&proj) {
            project_order.push(proj.clone());
        }
        grouped.entry(proj).or_default().push(action);
    }

    for proj_name in &project_order {
        let actions = match grouped.get(proj_name) {
            Some(a) => a,
            None => continue,
        };

        // Project header
        let header_icon = if proj_name == "Unlinked" { "\u{25CB}" } else { "\u{25A0}" };
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} {} ", header_icon, proj_name),
                Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("({})", actions.len()),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        for action in actions {
            item_line_starts.push(lines.len());
            let is_selected = item_idx == selected;
            let marker = if is_selected { "\u{25B8} " } else { "  " };
            let base_style = if is_selected {
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };

            let status_dot = match action.status.as_str() {
                "pending" => "\u{25CF}",
                "in_progress" => "\u{25D0}",
                "done" => "\u{2713}",
                "cancelled" => "\u{2717}",
                _ => "\u{25CB}",
            };

            // Show days left to deadline, or age if no deadline
            let (time_str, time_color) = match &action.deadline {
                Some(d) if !d.is_empty() => {
                    if let Some(days_left) = parse_days_left(d) {
                        if days_left < 0 {
                            (format!(" {}d overdue", -days_left), Color::Red)
                        } else if days_left == 0 {
                            (" TODAY".to_string(), Color::Red)
                        } else if days_left <= 3 {
                            (format!(" {}d left", days_left), Color::Yellow)
                        } else {
                            (format!(" {}d left", days_left), Color::DarkGray)
                        }
                    } else {
                        (format!(" due:{}", d), Color::Yellow)
                    }
                }
                _ => {
                    if action.age_days > 0 {
                        (format!(" {}d old", action.age_days), Color::DarkGray)
                    } else {
                        (" new".to_string(), Color::DarkGray)
                    }
                }
            };

            let is_expanded = state.expanded.contains(&item_idx);

            lines.push(Line::from(vec![
                Span::styled(marker.to_string(), base_style),
                Span::styled(format!("{} ", status_dot), Style::default().fg(status_color(&action.status))),
                Span::styled(
                    truncate(&action.description, area.width.saturating_sub(30) as usize),
                    base_style,
                ),
                Span::styled(time_str, Style::default().fg(time_color)),
            ]));

            if is_expanded {
                if let Some(ref dl) = action.deadline {
                    lines.push(Line::from(Span::styled(
                        format!("    Deadline: {}", dl),
                        Style::default().fg(Color::Yellow),
                    )));
                }
                lines.push(Line::from(Span::styled(
                    format!("    Status: {} | Age: {} days", action.status, action.age_days),
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(""));
            }

            item_idx += 1;
        }

        // Spacing between project groups
        lines.push(Line::from(""));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No actions found. Use helix_create_task to add tasks.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let selected_line = item_line_starts.get(selected).copied().unwrap_or(0);
    let half_height = area.height as usize / 2;
    let scroll = if selected_line > half_height {
        (selected_line - half_height) as u16
    } else {
        0
    };
    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), area);
}

fn render_detail_pane(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    data: &ExplorerData,
    state: &ExplorerState,
    theme: &Theme,
) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_set(theme.border_set())
        .border_style(Style::default().fg(theme.dimmed))
        .style(Style::default().bg(Color::Black));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let entity = match find_entity_by_name(data, &state.detail_entity_id) {
        Some(e) => e,
        None => {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!("  Entity '{}' not found", state.detail_entity_id),
                    Style::default().fg(Color::DarkGray),
                ))),
                inner,
            );
            return;
        }
    };

    let backlinks = find_backlinks(data, &entity.name);
    let total_links = backlinks.len() + entity.links_to.len();
    // Clamp cursor to valid range
    let detail_cursor = if total_links == 0 { 0 } else { state.detail_cursor.min(total_links - 1) };
    let max_w = inner.width.saturating_sub(2) as usize;
    let mut lines: Vec<Line> = Vec::new();

    // 1. Name
    lines.push(Line::from(Span::styled(
        format!(" {}", entity.name),
        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
    )));

    // 2. Type + scope
    let badge = if entity.scope.is_empty() {
        format!(" {}", entity.entity_type)
    } else {
        format!(" {} {} \u{00B7} {}", scope_icon(&entity.scope), entity.entity_type, entity.scope)
    };
    lines.push(Line::from(Span::styled(badge, Style::default().fg(Color::DarkGray))));

    // 3. Key fields
    if !entity.status.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(" Status: {}", entity.status),
            Style::default().fg(Color::Gray),
        )));
    }
    if let Some(ref dl) = entity.deadline {
        if !dl.is_empty() {
            let label = if entity.entity_type.contains("person") || entity.entity_type.contains("client") {
                "Last contact"
            } else {
                "Deadline"
            };
            lines.push(Line::from(Span::styled(
                format!(" {}: {}", label, dl),
                Style::default().fg(Color::Gray),
            )));
        }
    }

    // 4. Context
    if !entity.context.is_empty() {
        lines.push(Line::from(""));
        // Word-wrap context into multiple lines
        let words: Vec<&str> = entity.context.split_whitespace().collect();
        let mut current_line = String::from(" ");
        let mut line_count = 0;
        for word in words {
            if current_line.len() + word.len() + 1 > max_w && current_line.len() > 1 {
                lines.push(Line::from(Span::styled(
                    current_line.clone(),
                    Style::default().fg(Color::Gray),
                )));
                current_line = String::from(" ");
                line_count += 1;
                if line_count >= 4 { break; }
            }
            if current_line.len() > 1 { current_line.push(' '); }
            current_line.push_str(word);
        }
        if line_count < 4 && current_line.len() > 1 {
            lines.push(Line::from(Span::styled(current_line, Style::default().fg(Color::Gray))));
        }
    }

    lines.push(Line::from(""));

    // 5. Backlinks
    let mut link_idx: usize = 0;

    if !backlinks.is_empty() {
        lines.push(Line::from(Span::styled(
            " \u{2190} LINKED FROM",
            Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
        )));
        for bl in &backlinks {
            let is_cursor = detail_cursor == link_idx;
            let marker = if is_cursor { " \u{25B8} " } else { "   " };
            let style = if is_cursor {
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            let label = truncate(&format!("{}: {}", bl.relation_type, bl.entity_name), max_w.saturating_sub(3));
            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(label, style),
            ]));
            link_idx += 1;
        }
    }

    // 6. Outgoing links
    if !entity.links_to.is_empty() {
        lines.push(Line::from(Span::styled(
            " \u{2192} LINKS TO",
            Style::default().fg(theme.secondary).add_modifier(Modifier::BOLD),
        )));
        for (rel_type, target) in &entity.links_to {
            let is_cursor = detail_cursor == link_idx;
            let marker = if is_cursor { " \u{25B8} " } else { "   " };
            let style = if is_cursor {
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            let label = truncate(&format!("{}: {}", rel_type, target), max_w.saturating_sub(3));
            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(label, style),
            ]));
            link_idx += 1;
        }
    }

    // Breadcrumb hint
    if !state.detail_history.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(" [Bksp] back ({} in history)", state.detail_history.len()),
            Style::default().fg(theme.dimmed),
        )));
    }

    let scroll = if detail_cursor >= inner.height as usize {
        (detail_cursor - inner.height as usize / 2) as u16
    } else {
        0
    };

    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
}

fn render_search_bar(frame: &mut Frame, area: ratatui::layout::Rect, state: &ExplorerState, theme: &Theme) {
    let line = Line::from(vec![
        Span::styled(" / ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(&state.search_query, Style::default().fg(theme.text)),
        Span::styled("\u{258E}", Style::default().fg(theme.accent)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_command_bar(frame: &mut Frame, area: ratatui::layout::Rect, state: &ExplorerState, theme: &Theme) {
    let line = Line::from(vec![
        Span::styled(" : ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(&state.command_input, Style::default().fg(theme.text)),
        Span::styled("\u{258E}", Style::default().fg(Color::Yellow)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_footer(frame: &mut Frame, area: ratatui::layout::Rect, state: &ExplorerState, data: &ExplorerData, theme: &Theme) {
    // Toast takes priority
    if let Some(until) = state.toast_until {
        if Instant::now() < until {
            let color = match state.toast_level {
                ToastLevel::Success => Color::Green,
                ToastLevel::Error => Color::Red,
                ToastLevel::Info => theme.accent,
            };
            let prefix = if state.busy { " ... " } else { " > " };
            let line = Line::from(Span::styled(
                format!("{}{}", prefix, state.toast_message),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));
            frame.render_widget(Paragraph::new(line), area);
            return;
        }
    }

    let text = if state.command_active {
        " [Esc] cancel  [Enter] execute \u{2014} task <title> | note <text> | done <name> | search <query>".to_string()
    } else if state.search_active {
        let (filtered, total) = count_filtered_for_tab(data, state.active_tab, &state.search_query);
        format!(" [Esc] cancel  [Enter] apply \u{2014} {} of {} results", filtered, total)
    } else if state.detail_open {
        let back = if state.detail_history.is_empty() { "" } else { "  [Bksp] back" };
        format!(" [\u{2191}\u{2193}] relations  [Enter] navigate  [Esc/\u{2190}] close pane{}", back)
    } else {
        // Context-sensitive action hints
        let actions = match state.active_tab {
            0 => "  [p] status",      // Projects: cycle status
            4 => "  [d] done  [p] pri", // Actions: complete + priority
            _ => "",
        };
        let search_note = if !state.search_query.is_empty() {
            let (filtered, total) = count_filtered_for_tab(data, state.active_tab, &state.search_query);
            format!(" \u{2014} {} of {} shown", filtered, total)
        } else {
            String::new()
        };
        format!(
            " [Tab] tabs  [jk] nav  [Enter] expand  [l] detail  [/] search  [:] cmd{}{}{}",
            actions,
            "  [q] close",
            search_note,
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(text, Style::default().fg(theme.dimmed)))),
        area,
    );
}

fn count_filtered_for_tab(data: &ExplorerData, tab: usize, query: &str) -> (usize, usize) {
    match tab {
        0 => {
            let total = data.projects.len();
            let filtered = data.projects.iter().filter(|p| {
                matches_search(&p.name, query) || matches_search(&p.context, query) || matches_search(&p.scope, query)
            }).count();
            (filtered, total)
        }
        1 => {
            let total = data.people.len();
            let filtered = data.people.iter().filter(|p| {
                matches_search(&p.name, query) || matches_search(&p.role, query)
            }).count();
            (filtered, total)
        }
        2 => {
            let total = data.decisions.len();
            let filtered = data.decisions.iter().filter(|d| {
                matches_search(&d.name, query) || matches_search(&d.summary, query)
            }).count();
            (filtered, total)
        }
        3 => {
            let total = data.timeline.len();
            let filtered = data.timeline.iter().filter(|e| {
                matches_search(&e.summary, query) || matches_search(&e.source, query)
                    || e.entity_names.iter().any(|n| matches_search(n, query))
            }).count();
            (filtered, total)
        }
        4 => {
            let total = data.actions.len();
            let filtered = data.actions.iter().filter(|a| {
                matches_search(&a.description, query) || matches_search(&a.project, query)
            }).count();
            (filtered, total)
        }
        _ => (0, 0),
    }
}
