pub mod breathing_glow;
pub mod cosmic_eye;
pub mod fireflies;
pub mod fractal_plasma;
pub mod lava_lamp;
pub mod matrix_rain;

use crate::status::HelixState;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;

pub trait AmbientEffect {
    fn tick(&mut self, state: HelixState);
    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme, state: HelixState);
}
