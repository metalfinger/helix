use super::VisualizerStyle;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Braille waveform oscilloscope — renders audio frequency data as a smooth
/// high-resolution waveform using braille characters (each char = 2x4 pixel grid).
///
/// Braille dot positions in a character:
///   ⠁(0,0) ⠈(1,0)
///   ⠂(0,1) ⠐(1,1)
///   ⠄(0,2) ⠠(1,2)
///   ⡀(0,3) ⢀(1,3)
const BRAILLE_BASE: u32 = 0x2800;
const BRAILLE_DOTS: [[u32; 4]; 2] = [
    [0x01, 0x02, 0x04, 0x40], // left column: dots 1,2,3,7
    [0x08, 0x10, 0x20, 0x80], // right column: dots 4,5,6,8
];

pub struct ParticleField {
    /// Previous waveform for smooth transitions
    prev_wave: Vec<f32>,
    /// History of waveforms for trail effect
    history: Vec<Vec<f32>>,
}

impl ParticleField {
    pub fn new() -> Self {
        Self {
            prev_wave: Vec::new(),
            history: Vec::new(),
        }
    }

    /// Build a smooth waveform from frequency bands using sine synthesis
    fn bands_to_waveform(bands: &[f32], points: usize) -> Vec<f32> {
        let mut wave = vec![0.0f32; points];
        let n = bands.len();
        if n == 0 {
            return wave;
        }

        // Synthesize waveform: each band contributes a sine wave at its frequency
        for (i, &amp) in bands.iter().enumerate() {
            if amp < 0.01 {
                continue;
            }
            let freq = (i + 1) as f32 * 0.5; // frequency multiplier
            let phase = i as f32 * 0.7; // offset phases for visual interest
            for (j, sample) in wave.iter_mut().enumerate() {
                let t = j as f32 / points as f32 * std::f32::consts::TAU;
                *sample += amp * (t * freq + phase).sin();
            }
        }

        // Normalize to -1..1
        let max = wave.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        if max > 0.0 {
            for v in &mut wave {
                *v /= max;
            }
        }
        wave
    }

    /// Set a braille dot in a grid buffer
    fn set_dot(grid: &mut Vec<Vec<u32>>, grid_w: usize, grid_h: usize, px: usize, py: usize) {
        let cx = px / 2;
        let cy = py / 4;
        let dx = px % 2;
        let dy = py % 4;
        if cx < grid_w && cy < grid_h {
            grid[cy][cx] |= BRAILLE_DOTS[dx][dy];
        }
    }

    fn rgb(color: Color) -> (f32, f32, f32) {
        match color {
            Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
            _ => (128.0, 128.0, 128.0),
        }
    }
}

impl VisualizerStyle for ParticleField {
    fn name(&self) -> &str {
        "Waveform"
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

        // Braille pixel resolution: 2x wide, 4x tall per character
        let px_w = w * 2;
        let px_h = h * 4;

        // Generate current waveform
        let mut wave = Self::bands_to_waveform(bands, px_w);

        // Smooth transition from previous frame
        if self.prev_wave.len() == px_w {
            for (i, v) in wave.iter_mut().enumerate() {
                *v = self.prev_wave[i] * 0.3 + *v * 0.7;
            }
        }
        self.prev_wave = wave.clone();

        // Store history for trail effect (keep last 4 frames)
        self.history.push(wave.clone());
        if self.history.len() > 4 {
            self.history.remove(0);
        }

        let buf = frame.buffer_mut();

        // Render trail (older frames, dimmer)
        for (age, hist_wave) in self.history.iter().rev().enumerate().skip(1) {
            let fade = 1.0 / (age as f32 + 1.0) * 0.3;
            let (r, g, b) = Self::rgb(theme.dimmed);

            let mut grid = vec![vec![0u32; w]; h];
            for px in 0..px_w {
                let val = hist_wave.get(px).copied().unwrap_or(0.0);
                // Map -1..1 to pixel y with some padding
                let py = ((1.0 - val) * 0.5 * (px_h - 1) as f32) as usize;
                let py = py.min(px_h - 1);
                Self::set_dot(&mut grid, w, h, px, py);
            }
            for cy in 0..h {
                for cx in 0..w {
                    if grid[cy][cx] != 0 {
                        let x = area.x + cx as u16;
                        let y = area.y + cy as u16;
                        if x < area.x + area.width && y < area.y + area.height {
                            let ch = char::from_u32(BRAILLE_BASE + grid[cy][cx]).unwrap_or(' ');
                            buf[(x, y)].set_char(ch);
                            buf[(x, y)].set_fg(Color::Rgb(
                                (r * fade) as u8,
                                (g * fade) as u8,
                                (b * fade) as u8,
                            ));
                        }
                    }
                }
            }
        }

        // Render main waveform (current frame, bright)
        let mut grid = vec![vec![0u32; w]; h];
        // Also track which character cells are used and the y-positions for coloring
        let mut cell_max_energy = vec![vec![0.0f32; w]; h];

        for px in 0..px_w {
            let val = wave.get(px).copied().unwrap_or(0.0);
            let py = ((1.0 - val) * 0.5 * (px_h - 1) as f32) as usize;
            let py = py.min(px_h - 1);
            Self::set_dot(&mut grid, w, h, px, py);

            // Track energy for color intensity
            let cx = px / 2;
            let cy = py / 4;
            if cx < w && cy < h {
                let energy = val.abs();
                if energy > cell_max_energy[cy][cx] {
                    cell_max_energy[cy][cx] = energy;
                }
            }

            // Thicken the line: also draw ±1 pixel for filled look
            if py > 0 {
                Self::set_dot(&mut grid, w, h, px, py - 1);
            }
            if py + 1 < px_h {
                Self::set_dot(&mut grid, w, h, px, py + 1);
            }
        }

        // Render center line (very dim)
        let center_py = px_h / 2;
        let center_cy = center_py / 4;
        if center_cy < h {
            let center_dy = center_py % 4;
            for cx in 0..w {
                if grid[center_cy][cx] == 0 {
                    // Only draw center line where waveform isn't
                    let dot = BRAILLE_DOTS[0][center_dy] | BRAILLE_DOTS[1][center_dy];
                    let x = area.x + cx as u16;
                    let y = area.y + center_cy as u16;
                    if x < area.x + area.width && y < area.y + area.height {
                        let ch = char::from_u32(BRAILLE_BASE + dot).unwrap_or(' ');
                        let (r, g, b) = Self::rgb(theme.dimmed);
                        buf[(x, y)].set_char(ch);
                        buf[(x, y)].set_fg(Color::Rgb(
                            (r * 0.2) as u8,
                            (g * 0.2) as u8,
                            (b * 0.2) as u8,
                        ));
                    }
                }
            }
        }

        // Write braille characters for main waveform
        for cy in 0..h {
            for cx in 0..w {
                if grid[cy][cx] != 0 {
                    let x = area.x + cx as u16;
                    let y = area.y + cy as u16;
                    if x < area.x + area.width && y < area.y + area.height {
                        let ch = char::from_u32(BRAILLE_BASE + grid[cy][cx]).unwrap_or(' ');

                        // Single color with brightness based on amplitude
                        let energy = cell_max_energy[cy][cx];
                        let brightness = 0.4 + energy * 0.6;
                        let (r, g, b) = Self::rgb(theme.secondary);

                        let color = Color::Rgb(
                            (r * brightness).min(255.0) as u8,
                            (g * brightness).min(255.0) as u8,
                            (b * brightness).min(255.0) as u8,
                        );

                        buf[(x, y)].set_char(ch);
                        buf[(x, y)].set_fg(color);
                    }
                }
            }
        }
    }
}
