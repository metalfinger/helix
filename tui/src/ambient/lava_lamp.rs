use crate::status::HelixState;
use crate::theme::Theme;
use crate::ambient::AmbientEffect;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget as RatatuiWidget;

const MAX_BLOBS: usize = 6;
const DEFAULT_BLOB_COUNT: usize = 4;

struct Blob {
    x: f32,
    y: f32,
    radius: f32,
    base_radius: f32,
    vx: f32,
    vy: f32,
    color: Color,
    breath_phase: f32,
}

pub struct LavaLamp {
    blobs: Vec<Blob>,
    width: f32,
    height: f32,
    seed: u64,
}

impl LavaLamp {
    pub fn new() -> Self {
        let mut lamp = Self {
            blobs: Vec::new(),
            width: 80.0,
            height: 24.0,
            seed: 98765,
        };
        lamp.init_blobs(DEFAULT_BLOB_COUNT);
        lamp
    }

    fn xorshift(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }

    fn random_f32(&mut self) -> f32 {
        (self.xorshift() % 10000) as f32 / 10000.0
    }

    fn init_blobs(&mut self, count: usize) {
        self.blobs.clear();
        let default_colors = [
            Color::Rgb(180, 60, 60),   // warm red
            Color::Rgb(60, 120, 180),  // cool blue
            Color::Rgb(160, 80, 180),  // purple
            Color::Rgb(60, 160, 100),  // green
            Color::Rgb(180, 140, 40),  // amber
            Color::Rgb(80, 160, 180),  // teal
        ];
        for i in 0..count.min(MAX_BLOBS) {
            let x = self.random_f32() * self.width;
            let y = self.random_f32() * self.height;
            let base_radius = 3.0 + self.random_f32() * 2.0;
            let vx = (self.random_f32() - 0.5) * 0.2;
            let vy = (self.random_f32() - 0.5) * 0.15;
            let breath_phase = self.random_f32() * std::f32::consts::TAU;
            self.blobs.push(Blob {
                x,
                y,
                radius: base_radius,
                base_radius,
                vx,
                vy,
                color: default_colors[i % default_colors.len()],
                breath_phase,
            });
        }
    }

    /// Set blob colors from active session colors. One blob per session, up to 6.
    /// Extra blobs beyond session count use default accent colors.
    pub fn set_session_colors(&mut self, colors: Vec<Color>) {
        let target_count = colors.len().clamp(DEFAULT_BLOB_COUNT, MAX_BLOBS);

        // Grow or shrink blob list
        while self.blobs.len() < target_count {
            let x = self.random_f32() * self.width;
            let y = self.random_f32() * self.height;
            let base_radius = 3.0 + self.random_f32() * 2.0;
            let vx = (self.random_f32() - 0.5) * 0.2;
            let vy = (self.random_f32() - 0.5) * 0.15;
            let breath_phase = self.random_f32() * std::f32::consts::TAU;
            self.blobs.push(Blob {
                x,
                y,
                radius: base_radius,
                base_radius,
                vx,
                vy,
                color: Color::Rgb(100, 100, 100),
                breath_phase,
            });
        }
        while self.blobs.len() > target_count {
            self.blobs.pop();
        }

        // Assign colors
        for (i, blob) in self.blobs.iter_mut().enumerate() {
            if i < colors.len() {
                blob.color = colors[i];
            }
            // else keep existing color (default accent)
        }
    }

    pub fn set_size(&mut self, width: u16, height: u16) {
        self.width = width as f32;
        self.height = height as f32;
    }
}

impl AmbientEffect for LavaLamp {
    fn tick(&mut self, _state: HelixState) {
        let w = self.width;
        let h = self.height;
        if w < 1.0 || h < 1.0 {
            return;
        }

        for blob in &mut self.blobs {
            // Blob breathing — oscillate radius with sine wave ±0.5
            blob.breath_phase += 0.03;
            if blob.breath_phase > std::f32::consts::TAU {
                blob.breath_phase -= std::f32::consts::TAU;
            }
            blob.radius = blob.base_radius + blob.breath_phase.sin() * 0.5;

            // Slight random drift
            // We inline xorshift here to avoid borrow issues
            let seed = &mut (blob.x.to_bits() as u64).wrapping_add(blob.y.to_bits() as u64).wrapping_add(1);
            *seed ^= *seed << 13;
            *seed ^= *seed >> 7;
            *seed ^= *seed << 17;
            let rx = ((*seed % 1000) as f32 / 1000.0 - 0.5) * 0.02;
            *seed ^= *seed << 13;
            *seed ^= *seed >> 7;
            *seed ^= *seed << 17;
            let ry = ((*seed % 1000) as f32 / 1000.0 - 0.5) * 0.02;

            blob.vx += rx;
            blob.vy += ry;

            // Clamp velocity — slower for hypnotic drift
            blob.vx = blob.vx.clamp(-0.08, 0.08);
            blob.vy = blob.vy.clamp(-0.08, 0.08);

            blob.x += blob.vx;
            blob.y += blob.vy;

            // Bounce off edges with sine easing (decelerate near edge)
            let margin = blob.radius;
            if blob.x < margin {
                blob.vx = blob.vx.abs() * 0.8;
                blob.x = margin;
            } else if blob.x > w - margin {
                blob.vx = -blob.vx.abs() * 0.8;
                blob.x = w - margin;
            }
            if blob.y < margin * 0.5 {
                blob.vy = blob.vy.abs() * 0.8;
                blob.y = margin * 0.5;
            } else if blob.y > h - margin * 0.5 {
                blob.vy = -blob.vy.abs() * 0.8;
                blob.y = h - margin * 0.5;
            }
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, _theme: &Theme, _state: HelixState) {
        if self.blobs.is_empty() {
            return;
        }
        let widget = LavaLampWidget {
            blobs: &self.blobs,
        };
        frame.render_widget(widget, area);
    }
}

struct LavaLampWidget<'a> {
    blobs: &'a [Blob],
}

impl<'a> RatatuiWidget for LavaLampWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for sy in area.y..area.y + area.height {
            for sx in area.x..area.x + area.width {
                let px = sx as f32;
                let py = sy as f32;

                // Compute metaball field value and weighted color
                let mut field: f32 = 0.0;
                let mut total_r: f32 = 0.0;
                let mut total_g: f32 = 0.0;
                let mut total_b: f32 = 0.0;

                for blob in self.blobs {
                    let dx = px - blob.x;
                    let dy = (py - blob.y) * 2.0; // aspect ratio: terminal cells are ~2x taller than wide
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq < 0.01 {
                        // Avoid division by near-zero
                        field += blob.radius * blob.radius * 100.0;
                        let (cr, cg, cb) = color_to_rgb(blob.color);
                        let contribution = blob.radius * blob.radius * 100.0;
                        total_r += cr as f32 * contribution;
                        total_g += cg as f32 * contribution;
                        total_b += cb as f32 * contribution;
                        continue;
                    }
                    let contribution = blob.radius * blob.radius / dist_sq;
                    field += contribution;

                    let (cr, cg, cb) = color_to_rgb(blob.color);
                    total_r += cr as f32 * contribution;
                    total_g += cg as f32 * contribution;
                    total_b += cb as f32 * contribution;
                }

                if field < 0.08 {
                    continue; // transparent
                }

                // Smoother edge thresholds with half-block chars at edges
                let ch = if field > 0.55 {
                    '\u{2588}' // █ full
                } else if field > 0.35 {
                    '\u{2593}' // ▓ heavy
                } else if field > 0.18 {
                    '\u{2592}' // ▒ medium
                } else if field > 0.12 {
                    '\u{2591}' // ░ light
                } else {
                    // Half-block chars at the very edges for smoother gradient
                    // Use ▄ or ▀ based on position relative to nearest blob center
                    let mut nearest_blob_y = self.blobs[0].y;
                    let mut nearest_dist = f32::MAX;
                    for blob in self.blobs {
                        let d = ((px - blob.x).powi(2) + ((py - blob.y) * 2.0).powi(2)).sqrt();
                        if d < nearest_dist {
                            nearest_dist = d;
                            nearest_blob_y = blob.y;
                        }
                    }
                    if py < nearest_blob_y {
                        '\u{2584}' // ▄ bottom half (blob is below)
                    } else {
                        '\u{2580}' // ▀ top half (blob is above)
                    }
                };

                // Interpolated color from blob contributions
                let r = (total_r / field).min(255.0) as u8;
                let g = (total_g / field).min(255.0) as u8;
                let b = (total_b / field).min(255.0) as u8;

                // Blob merge glow — boost brightness when blobs overlap
                let merge_mult = if field > 0.7 { 1.3 } else { 1.0 };

                // Dim the output so it stays subtle as a background (0.5 for more visibility)
                let dim = 0.5 * merge_mult;
                let r = ((r as f32 * dim).min(255.0)) as u8;
                let g = ((g as f32 * dim).min(255.0)) as u8;
                let b = ((b as f32 * dim).min(255.0)) as u8;

                buf.cell_mut((sx, sy))
                    .map(|cell| {
                        cell.set_char(ch);
                        cell.set_style(Style::default().fg(Color::Rgb(r, g, b)));
                    });
            }
        }
    }
}

fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (128, 128, 128),
    }
}
