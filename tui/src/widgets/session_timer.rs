use crate::status::SessionStatus;
use crate::theme::Theme;
use crate::widgets::Widget;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::text::{Line, Span};

pub struct SessionTimer;

impl Widget for SessionTimer {
    fn render(&self, frame: &mut Frame, area: Rect, status: &SessionStatus, theme: &Theme, _tick: u64) {
        let block = Block::default()
            .title(" Session ")
            .borders(Borders::ALL)
            .border_set(theme.border_set())
            .border_style(Style::default().fg(theme.dimmed));

        let total = fmt_duration(status.session.duration_ms);
        let api = fmt_duration(status.session.api_duration_ms);
        let idle = fmt_duration(
            status.session.duration_ms.saturating_sub(status.session.api_duration_ms)
        );

        let lines_add = status.activity.lines_added;
        let lines_rm = status.activity.lines_removed;

        let mut lines = vec![
            Line::from(vec![
                Span::styled(" Total  ", Style::default().fg(theme.dimmed)),
                Span::styled(&total, Style::default().fg(theme.primary)),
            ]),
            Line::from(vec![
                Span::styled(" API    ", Style::default().fg(theme.dimmed)),
                Span::styled(&api, Style::default().fg(theme.secondary)),
            ]),
            Line::from(vec![
                Span::styled(" Idle   ", Style::default().fg(theme.dimmed)),
                Span::styled(&idle, Style::default().fg(theme.dimmed)),
            ]),
        ];

        if lines_add > 0 || lines_rm > 0 {
            let mut parts = vec![Span::styled(" Lines  ", Style::default().fg(theme.dimmed))];
            if lines_add > 0 {
                parts.push(Span::styled(format!("+{}", lines_add), Style::default().fg(theme.success)));
                parts.push(Span::styled(" ", Style::default()));
            }
            if lines_rm > 0 {
                parts.push(Span::styled(format!("-{}", lines_rm), Style::default().fg(theme.error)));
            }
            lines.push(Line::from(parts));
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}

fn fmt_duration(ms: u64) -> String {
    let s = ms / 1000;
    let m = s / 60;
    let h = m / 60;
    if h > 0 {
        format!("{:02}h {:02}m {:02}s", h, m % 60, s % 60)
    } else if m > 0 {
        format!("{:02}m {:02}s", m, s % 60)
    } else {
        format!("{:02}s", s)
    }
}
