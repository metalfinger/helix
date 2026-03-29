pub mod context_gauge;
pub mod finance_overlay;
pub mod history_overlay;
pub mod memory_explorer;
pub mod memory_panel;
pub mod session_timer;
pub mod system_monitor;
pub mod activity_feed;
pub mod visualizer;

use crate::status::SessionStatus;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;

pub trait Widget {
    fn render(&self, frame: &mut Frame, area: Rect, status: &SessionStatus, theme: &Theme, tick: u64);
}
