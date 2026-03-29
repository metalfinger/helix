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

/// Kaleidoscope: 8-fold symmetric morphing patterns using braille characters.
/// Shape: radius(θ) = base + Σ band[i] * sin(θ * (i+1) + rotation)
pub struct Kaleidoscope {
    rotation: f32,
    prev_energy: f32,
}

impl Kaleidoscope {
    pub fn new() -> Self {
        Self {
            rotation: 0.0,
            prev_energy: 0.0,
        }
    }

    fn rgb(color: Color) -> (f32, f32, f32) {
        match color {
            Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
            _ => (128.0, 128.0, 128.0),
        }
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

    fn shape_radius(angle: f32, bands: &[f32], base: f32, rotation: f32) -> f32 {
        let mut r = base;
        for (i, &amp) in bands.iter().enumerate() {
            if amp < 0.005 { continue; }
            let harmonic = (i + 1) as f32;
            r += amp * base * 0.5 * (angle * harmonic + rotation).sin();
        }
        r.max(0.0)
    }

    fn energy(bands: &[f32]) -> f32 {
        if bands.is_empty() { return 0.0; }
        let sum: f32 = bands.iter().map(|v| v * v).sum();
        (sum / bands.len() as f32).sqrt()
    }
}

impl VisualizerStyle for Kaleidoscope {
    fn name(&self) -> &str {
        "Kaleidoscope"
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
        let cx_f = px_w as f32 * 0.5;
        let cy_f = px_h as f32 * 0.5;

        // Base radius: fit within the available space
        // Braille pixels are roughly square, so use min dimension directly
        let base_radius = (px_w as f32).min(px_h as f32) * 0.35;

        let cur_energy = Self::energy(bands);
        let smooth_energy = self.prev_energy * 0.85 + cur_energy * 0.15;
        self.prev_energy = smooth_energy;

        let rotation_speed = 0.005 + smooth_energy * 0.035;
        self.rotation += rotation_speed;
        if self.rotation > std::f32::consts::TAU {
            self.rotation -= std::f32::consts::TAU;
        }

        const SYMMETRY: u32 = 8;
        let sector_angle = std::f32::consts::TAU / SYMMETRY as f32;

        let mut grid = vec![vec![0u32; w]; h];
        let mut cell_edge_ratio = vec![vec![0.0f32; w]; h];

        for py in 0..px_h {
            for px in 0..px_w {
                let dx = px as f32 - cx_f;
                let dy = py as f32 - cy_f;
                // No aspect correction needed — braille pixels are roughly square
                let dist = (dx * dx + dy * dy).sqrt();

                let mut angle = dy.atan2(dx);
                if angle < 0.0 { angle += std::f32::consts::TAU; }
                let folded_angle = angle % sector_angle;

                let r_shape = Self::shape_radius(folded_angle, bands, base_radius, self.rotation);

                if dist <= r_shape {
                    Self::set_dot(&mut grid, w, h, px, py);
                    let ratio = if r_shape > 0.0 { dist / r_shape } else { 0.0 };
                    let gcx = px / 2;
                    let gcy = py / 4;
                    if gcx < w && gcy < h && ratio > cell_edge_ratio[gcy][gcx] {
                        cell_edge_ratio[gcy][gcx] = ratio;
                    }
                }
            }
        }

        let buf = frame.buffer_mut();
        let (base_r, base_g, base_b) = Self::rgb(theme.secondary);

        for cy in 0..h {
            for cx in 0..w {
                if grid[cy][cx] == 0 { continue; }
                let x = area.x + cx as u16;
                let y = area.y + cy as u16;
                if x >= area.x + area.width || y >= area.y + area.height { continue; }

                let ch = char::from_u32(BRAILLE_BASE + grid[cy][cx]).unwrap_or(' ');
                let edge_ratio = cell_edge_ratio[cy][cx];
                let brightness = 0.2 + edge_ratio * 0.8;

                let color = Color::Rgb(
                    (base_r * brightness).min(255.0) as u8,
                    (base_g * brightness).min(255.0) as u8,
                    (base_b * brightness).min(255.0) as u8,
                );

                buf[(x, y)].set_char(ch);
                buf[(x, y)].set_fg(color);
            }
        }
    }
}
