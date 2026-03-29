use crate::status::HelixState;
use crate::theme::Theme;
use crate::ambient::AmbientEffect;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget as RatatuiWidget;

/// Fractal plasma — port of kishimisu's Shadertoy shader (mtyGWy)
/// Iterative domain-repetition with IQ cosine palette
pub struct FractalPlasma {
    time: f32,
    speed: f32,
}

impl FractalPlasma {
    pub fn new() -> Self {
        Self {
            time: 0.0,
            speed: 0.066, // ~1 second per 15 ticks
        }
    }
}

impl AmbientEffect for FractalPlasma {
    fn tick(&mut self, _state: HelixState) {
        self.time += self.speed;
    }

    fn render(&self, frame: &mut Frame, area: Rect, _theme: &Theme, _state: HelixState) {
        let widget = FractalPlasmaWidget { time: self.time };
        frame.render_widget(widget, area);
    }
}

struct FractalPlasmaWidget {
    time: f32,
}

/// IQ cosine palette: a + b * cos(TAU * (c*t + d))
/// https://iquilezles.org/articles/palettes/
fn palette(t: f32) -> (f32, f32, f32) {
    let tau = std::f32::consts::TAU;
    let dr = 0.263;
    let dg = 0.416;
    let db = 0.557;
    let r = 0.5 + 0.5 * (tau * (t + dr)).cos();
    let g = 0.5 + 0.5 * (tau * (t + dg)).cos();
    let b = 0.5 + 0.5 * (tau * (t + db)).cos();
    (r, g, b)
}

impl RatatuiWidget for FractalPlasmaWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let w = area.width as f32;
        let h = area.height as f32;
        // Use height * 2 for aspect correction (terminal cells ~2:1)
        let res_y = h * 2.0;
        let time = self.time;

        for sy in area.y..area.y + area.height {
            for sx in area.x..area.x + area.width {
                let px = sx as f32 - area.x as f32;
                let py = sy as f32 - area.y as f32;

                // Normalize to centered coords: (fragCoord * 2.0 - iResolution) / iResolution.y
                let mut ux = (px * 2.0 - w) / res_y;
                let mut uy = (py * 2.0 - h) / h; // no extra aspect here, already corrected

                let ux0 = ux;
                let uy0 = uy;

                let mut final_r: f32 = 0.0;
                let mut final_g: f32 = 0.0;
                let mut final_b: f32 = 0.0;

                // 4 iterations of fractal domain repetition
                for i in 0..4 {
                    // fract(uv * 1.5) - 0.5
                    ux = (ux * 1.5).fract() - 0.5;
                    uy = (uy * 1.5).fract() - 0.5;

                    // Handle negative fract (Rust fract can return negative for negative inputs)
                    if ux < -0.5 { ux += 1.0; }
                    if uy < -0.5 { uy += 1.0; }

                    // d = length(uv) * exp(-length(uv0))
                    let len_uv = (ux * ux + uy * uy).sqrt();
                    let len_uv0 = (ux0 * ux0 + uy0 * uy0).sqrt();
                    let mut d = len_uv * (-len_uv0).exp();

                    // Palette color from distance to origin + iteration offset + time
                    let (cr, cg, cb) = palette(len_uv0 + i as f32 * 0.4 + time * 0.4);

                    // d = sin(d*8 + iTime) / 8
                    d = (d * 8.0 + time).sin() / 8.0;
                    d = d.abs();

                    // d = pow(0.01 / d, 1.2)
                    d = (0.01 / d.max(0.0001)).powf(1.2);

                    // Accumulate: finalColor += col * d
                    final_r += cr * d;
                    final_g += cg * d;
                    final_b += cb * d;
                }

                // Dim for background use (adjustable)
                let dim = 0.35;
                let r = (final_r * dim).clamp(0.0, 1.0);
                let g = (final_g * dim).clamp(0.0, 1.0);
                let b = (final_b * dim).clamp(0.0, 1.0);

                // Brightness for glyph selection
                let brightness = 0.2126 * r + 0.7152 * g + 0.0722 * b;

                // Skip very dark cells — keep background transparent
                if brightness < 0.01 {
                    continue;
                }

                let ch = if brightness > 0.6 {
                    '█'
                } else if brightness > 0.4 {
                    '▓'
                } else if brightness > 0.25 {
                    '▒'
                } else if brightness > 0.12 {
                    '░'
                } else {
                    '·'
                };

                let cr = (r * 255.0) as u8;
                let cg = (g * 255.0) as u8;
                let cb = (b * 255.0) as u8;

                buf.cell_mut((sx, sy))
                    .map(|cell| {
                        cell.set_char(ch);
                        cell.set_style(Style::default().fg(Color::Rgb(cr, cg, cb)));
                    });
            }
        }
    }
}
