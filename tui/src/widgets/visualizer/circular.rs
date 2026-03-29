use super::VisualizerStyle;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

const BRAILLE_BASE: u32 = 0x2800;
const BRAILLE_DOTS: [[u32; 4]; 2] = [
    [0x01, 0x02, 0x04, 0x40],
    [0x08, 0x10, 0x20, 0x80],
];

const MAX_RINGS: usize = 5;

/// Circular pulse visualizer with expanding beat rings.
/// Uses braille characters with proper terminal aspect ratio correction.
pub struct Circular {
    base_radius: f32,
    prev_energy: f32,
    rings: Vec<(f32, f32)>, // (radius, brightness)
}

impl Circular {
    pub fn new() -> Self {
        Self {
            base_radius: 4.0,
            prev_energy: 0.0,
            rings: Vec::with_capacity(MAX_RINGS),
        }
    }

    fn rgb(color: Color) -> (f32, f32, f32) {
        match color {
            Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
            _ => (128.0, 128.0, 128.0),
        }
    }

    fn bass_energy(bands: &[f32]) -> f32 {
        let n = bands.len();
        if n == 0 { return 0.0; }
        let bass_end = (n / 4).max(1);
        let bass: f32 = bands[..bass_end].iter().sum::<f32>() / bass_end as f32;
        let overall: f32 = bands.iter().sum::<f32>() / n as f32;
        bass * 0.7 + overall * 0.3
    }

    fn set_dot(grid: &mut [Vec<u32>], grid_w: usize, grid_h: usize, px: usize, py: usize) {
        let cx = px / 2;
        let cy = py / 4;
        let dx = px % 2;
        let dy = py % 4;
        if cx < grid_w && cy < grid_h {
            grid[cy][cx] |= BRAILLE_DOTS[dx][dy];
        }
    }

    /// Draw circle with proper aspect correction.
    /// Terminal chars are roughly 1:2 (w:h), braille is 2x4 dots.
    /// In braille pixel space: px_w = w*2, px_h = h*4
    /// Actual visual aspect: each braille pixel is (char_w/2) wide and (char_h/4) tall.
    /// Since char_h ≈ 2*char_w, pixel_h ≈ 2*char_w/4 = char_w/2 = pixel_w.
    /// So braille pixels are actually roughly square! No correction needed.
    fn draw_circle(
        grid: &mut [Vec<u32>],
        grid_w: usize,
        grid_h: usize,
        cx_px: f32,
        cy_px: f32,
        radius: f32,
    ) {
        if radius < 1.0 { return; }
        let steps = ((radius * std::f32::consts::TAU).ceil() as usize).max(16);
        for i in 0..steps {
            let angle = i as f32 / steps as f32 * std::f32::consts::TAU;
            let px = (cx_px + angle.cos() * radius).round() as isize;
            let py = (cy_px + angle.sin() * radius).round() as isize;
            if px >= 0 && py >= 0 {
                Self::set_dot(grid, grid_w, grid_h, px as usize, py as usize);
            }
        }
    }

    fn make_color(brightness: f32, theme: &Theme) -> Color {
        let (r, g, b) = Self::rgb(theme.secondary);
        let f = brightness.clamp(0.0, 1.0);
        Color::Rgb(
            (r * f).min(255.0) as u8,
            (g * f).min(255.0) as u8,
            (b * f).min(255.0) as u8,
        )
    }
}

impl VisualizerStyle for Circular {
    fn name(&self) -> &str {
        "Circular Pulse"
    }

    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        bands: &[f32],
        theme: &Theme,
        _tick: u64,
    ) {
        if area.width < 4 || area.height < 3 || bands.is_empty() {
            return;
        }

        let w = area.width as usize;
        let h = area.height as usize;
        let px_w = w * 2;
        let px_h = h * 4;
        let cx = px_w as f32 / 2.0;
        let cy = px_h as f32 / 2.0;

        let energy = Self::bass_energy(bands);
        let beat = energy > self.prev_energy * 1.5 && energy > 0.1;
        self.prev_energy = self.prev_energy * 0.8 + energy * 0.2;

        // Max radius: fit within the smaller pixel dimension
        let max_r = (px_w as f32 * 0.45).min(px_h as f32 * 0.45);
        let target = 3.0 + energy * max_r * 0.6;
        self.base_radius = (self.base_radius * 0.7 + target * 0.3).clamp(3.0, max_r * 0.5);

        if beat && self.rings.len() < MAX_RINGS {
            self.rings.push((self.base_radius, 1.0));
        }

        // Advance rings
        self.rings.retain_mut(|(r, b)| {
            *r += 1.2;
            *b -= 0.03;
            *r < max_r && *b > 0.0
        });

        let buf = frame.buffer_mut();

        // Draw rings (dim)
        for &(ring_r, ring_b) in &self.rings {
            let mut grid = vec![vec![0u32; w]; h];
            Self::draw_circle(&mut grid, w, h, cx, cy, ring_r);
            let color = Self::make_color(ring_b * 0.6, theme);
            for cy_cell in 0..h {
                for cx_cell in 0..w {
                    if grid[cy_cell][cx_cell] != 0 {
                        let x = area.x + cx_cell as u16;
                        let y = area.y + cy_cell as u16;
                        if x < area.x + area.width && y < area.y + area.height {
                            let ch = char::from_u32(BRAILLE_BASE + grid[cy_cell][cx_cell]).unwrap_or(' ');
                            buf[(x, y)].set_char(ch);
                            buf[(x, y)].set_fg(color);
                        }
                    }
                }
            }
        }

        // Draw core circle (bright) + inner fill circle
        let mut core_grid = vec![vec![0u32; w]; h];
        Self::draw_circle(&mut core_grid, w, h, cx, cy, self.base_radius);
        if self.base_radius > 3.0 {
            Self::draw_circle(&mut core_grid, w, h, cx, cy, self.base_radius * 0.7);
        }
        if self.base_radius > 5.0 {
            Self::draw_circle(&mut core_grid, w, h, cx, cy, self.base_radius * 0.4);
        }

        let core_color = Self::make_color(0.5 + energy * 0.5, theme);
        for cy_cell in 0..h {
            for cx_cell in 0..w {
                if core_grid[cy_cell][cx_cell] != 0 {
                    let x = area.x + cx_cell as u16;
                    let y = area.y + cy_cell as u16;
                    if x < area.x + area.width && y < area.y + area.height {
                        let ch = char::from_u32(BRAILLE_BASE + core_grid[cy_cell][cx_cell]).unwrap_or(' ');
                        buf[(x, y)].set_char(ch);
                        buf[(x, y)].set_fg(core_color);
                    }
                }
            }
        }
    }
}
