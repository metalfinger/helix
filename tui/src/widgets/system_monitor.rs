use crate::status::SessionStatus;
use crate::theme::Theme;
use crate::widgets::Widget;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::text::{Line, Span};
use sysinfo::System;
use std::sync::{Arc, Mutex, OnceLock};

static SYSTEM: OnceLock<Arc<Mutex<System>>> = OnceLock::new();

fn get_system() -> Arc<Mutex<System>> {
    SYSTEM.get_or_init(|| {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        Arc::new(Mutex::new(sys))
    }).clone()
}

pub struct SystemMonitor {
    last_refresh: std::cell::Cell<u64>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        Self {
            last_refresh: std::cell::Cell::new(0),
        }
    }
}

impl Widget for SystemMonitor {
    fn render(&self, frame: &mut Frame, area: Rect, _status: &SessionStatus, theme: &Theme, tick: u64) {
        let sys = get_system();
        let mut sys = sys.lock().unwrap();

        if tick.saturating_sub(self.last_refresh.get()) >= 15 {
            sys.refresh_cpu_all();
            sys.refresh_memory();
            self.last_refresh.set(tick);
        }

        let cpu = sys.global_cpu_usage();
        let mem_used = sys.used_memory();
        let mem_total = sys.total_memory();
        let mem_pct = if mem_total > 0 {
            (mem_used as f64 / mem_total as f64 * 100.0) as u32
        } else {
            0
        };

        let block = Block::default()
            .title(" System ")
            .borders(Borders::ALL)
            .border_set(theme.border_set())
            .border_style(Style::default().fg(theme.dimmed));

        let cpu_bar = make_bar(cpu as u32, 100, 20);
        let mem_bar = make_bar(mem_pct, 100, 20);

        let cpu_color = if cpu > 80.0 { theme.error } else if cpu > 50.0 { theme.warning } else { theme.success };
        let mem_color = if mem_pct > 80 { theme.error } else if mem_pct > 50 { theme.warning } else { theme.success };

        let lines = vec![
            Line::from(vec![
                Span::styled(" CPU ", Style::default().fg(theme.dimmed)),
                Span::styled(&cpu_bar, Style::default().fg(cpu_color)),
                Span::styled(format!(" {:.0}%", cpu), Style::default().fg(cpu_color)),
            ]),
            Line::from(vec![
                Span::styled(" RAM ", Style::default().fg(theme.dimmed)),
                Span::styled(&mem_bar, Style::default().fg(mem_color)),
                Span::styled(format!(" {}%", mem_pct), Style::default().fg(mem_color)),
            ]),
            Line::from(vec![
                Span::styled(
                    format!("      {} / {}", fmt_bytes(mem_used), fmt_bytes(mem_total)),
                    Style::default().fg(theme.dimmed),
                ),
            ]),
        ];

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}

fn make_bar(value: u32, max: u32, width: usize) -> String {
    let filled = (value as f64 / max as f64 * width as f64) as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "\u{2593}".repeat(filled), "\u{2591}".repeat(empty))
}

fn fmt_bytes(b: u64) -> String {
    if b >= 1_073_741_824 {
        format!("{:.1}GB", b as f64 / 1_073_741_824.0)
    } else if b >= 1_048_576 {
        format!("{:.0}MB", b as f64 / 1_048_576.0)
    } else {
        format!("{:.0}KB", b as f64 / 1024.0)
    }
}
