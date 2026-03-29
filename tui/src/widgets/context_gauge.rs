use crate::status::SessionStatus;
use crate::theme::Theme;
use crate::widgets::Widget;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Gauge};

pub struct ContextGauge;

impl Widget for ContextGauge {
    fn render(&self, frame: &mut Frame, area: Rect, status: &SessionStatus, theme: &Theme, tick: u64) {
        let pct = status.used_pct().min(100);

        let color = if pct >= 90 {
            theme.error
        } else if pct >= 70 {
            theme.warning
        } else if pct >= 50 {
            theme.primary
        } else {
            theme.success
        };

        let label_color = if pct >= 80 && tick % 20 < 10 {
            theme.accent
        } else {
            color
        };

        let total = status.tokens.input + status.tokens.output;
        let remaining = status.tokens.context_size.saturating_sub(total);
        let label = format!(
            "{}% | {} / {} | {} remaining",
            pct,
            fmt_tokens(total),
            fmt_tokens(status.tokens.context_size),
            fmt_tokens(remaining)
        );

        let block = Block::default()
            .title(" Context ")
            .borders(Borders::ALL)
            .border_set(theme.border_set())
            .border_style(Style::default().fg(theme.dimmed));

        let gauge = Gauge::default()
            .block(block)
            .gauge_style(Style::default().fg(color).bg(theme.background))
            .label(ratatui::text::Span::styled(label, Style::default().fg(label_color)))
            .ratio(pct as f64 / 100.0);

        frame.render_widget(gauge, area);
    }
}

fn fmt_tokens(t: u64) -> String {
    if t >= 1_000_000 {
        format!("{:.1}M", t as f64 / 1_000_000.0)
    } else if t >= 1_000 {
        format!("{:.1}K", t as f64 / 1_000.0)
    } else {
        t.to_string()
    }
}
