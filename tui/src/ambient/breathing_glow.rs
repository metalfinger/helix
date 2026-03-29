use crate::status::HelixState;
use crate::theme::Theme;
use crate::ambient::AmbientEffect;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub struct BreathingGlow {
    phase: f32,
    speed: f32,
}

impl BreathingGlow {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            speed: 0.025, // ~4 second cycle at 60fps
        }
    }

    pub fn brightness(&self) -> f32 {
        0.3 + 0.7 * self.phase.sin().abs()
    }

    /// Apply glow to a color by scaling RGB towards white
    pub fn apply_to_color(&self, color: Color) -> Color {
        let b = self.brightness();
        match color {
            Color::Rgb(r, g, bl) => Color::Rgb(
                (r as f32 * b).min(255.0) as u8,
                (g as f32 * b).min(255.0) as u8,
                (bl as f32 * b).min(255.0) as u8,
            ),
            other => other,
        }
    }
}

impl AmbientEffect for BreathingGlow {
    fn tick(&mut self, state: HelixState) {
        // Adjust breathing speed based on state
        self.speed = match state {
            HelixState::Thinking => 0.05,  // ~2s cycle — anxious breathing
            HelixState::Coding => 0.035,   // ~3s cycle — focused
            HelixState::Deep => 0.06,      // ~1.7s — intense
            HelixState::Critical => 0.08,  // ~1.3s — urgent
            HelixState::Error => 0.07,     // fast, alarmed
            _ => 0.025,                    // ~4s — calm idle
        };
        self.phase += self.speed;
        if self.phase > std::f32::consts::TAU {
            self.phase -= std::f32::consts::TAU;
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, _theme: &Theme, _state: HelixState) {
        // Modify border cells in the buffer to apply breathing effect
        let buf = frame.buffer_mut();
        let b = self.brightness();

        // Apply to top and bottom border rows
        for x in area.left()..area.right() {
            for y in [area.top(), area.bottom().saturating_sub(1)] {
                if y < buf.area.height && x < buf.area.width {
                    let cell = &mut buf[(x, y)];
                    if let Color::Rgb(r, g, bl) = cell.fg {
                        cell.set_fg(Color::Rgb(
                            (r as f32 * b).min(255.0) as u8,
                            (g as f32 * b).min(255.0) as u8,
                            (bl as f32 * b).min(255.0) as u8,
                        ));
                    }
                }
            }
        }
        // Apply to left and right border columns
        for y in area.top()..area.bottom() {
            for x in [area.left(), area.right().saturating_sub(1)] {
                if y < buf.area.height && x < buf.area.width {
                    let cell = &mut buf[(x, y)];
                    if let Color::Rgb(r, g, bl) = cell.fg {
                        cell.set_fg(Color::Rgb(
                            (r as f32 * b).min(255.0) as u8,
                            (g as f32 * b).min(255.0) as u8,
                            (bl as f32 * b).min(255.0) as u8,
                        ));
                    }
                }
            }
        }
    }
}
