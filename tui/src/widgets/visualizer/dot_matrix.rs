use super::VisualizerStyle;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Dot Matrix LED Display — simulates a physical hi-fi LED panel spectrum analyzer.
///
/// A grid of dots where `●` (lit) represents active amplitude and `·` (dim) represents
/// off segments.  Columns = frequency bands, rows = amplitude levels.  Includes:
/// - Peak hold per column with slow gravity decay
/// - Brightness gradient: brighter near the top of each lit column
/// - Monstercat smoothing for smooth band transitions
/// - Gravity-based rise/fall animation
pub struct DotMatrix {
    /// Current displayed bar heights with gravity applied (0.0 – 1.0)
    bars: Vec<f32>,
    /// Downward velocity for gravity drop-off per column
    velocity: Vec<f32>,
    /// Peak dot position per column (0.0 – 1.0)
    peaks: Vec<f32>,
    /// Peak hold countdown ticks per column
    peak_hold: Vec<u8>,
}

impl DotMatrix {
    pub fn new() -> Self {
        Self {
            bars: Vec::new(),
            velocity: Vec::new(),
            peaks: Vec::new(),
            peak_hold: Vec::new(),
        }
    }

    fn rgb(color: Color) -> (f32, f32, f32) {
        match color {
            Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
            _ => (128.0, 128.0, 128.0),
        }
    }

    fn scale_color(color: Color, brightness: f32) -> Color {
        let (r, g, b) = Self::rgb(color);
        Color::Rgb(
            (r * brightness).clamp(0.0, 255.0) as u8,
            (g * brightness).clamp(0.0, 255.0) as u8,
            (b * brightness).clamp(0.0, 255.0) as u8,
        )
    }

    /// Monstercat-style smoothing: adjacent columns pull each other toward a
    /// smooth curve instead of leaving jagged spikes.
    fn monstercat_smooth(bars: &mut [f32], strength: f32) {
        let n = bars.len();
        if n < 3 {
            return;
        }
        for i in 1..n {
            let prev = bars[i - 1] * strength;
            if bars[i] < prev {
                bars[i] = prev;
            }
        }
        for i in (0..n - 1).rev() {
            let next = bars[i + 1] * strength;
            if bars[i] < next {
                bars[i] = next;
            }
        }
    }
}

impl VisualizerStyle for DotMatrix {
    fn name(&self) -> &str {
        "Dot Matrix"
    }

    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        bands: &[f32],
        theme: &Theme,
        _tick: u64,
    ) {
        if area.width < 4 || area.height < 2 || bands.is_empty() {
            return;
        }

        let width = area.width as usize;
        let height = area.height as usize;

        // Each dot cell is 2 chars wide ("● " or "· "), so column count = width / 2.
        let col_count = (width / 2).max(2).min(48);
        let row_count = height; // one dot row per terminal row

        // Resample bands → col_count target values
        let mut target: Vec<f32> = (0..col_count)
            .map(|i| {
                let start = i * bands.len() / col_count;
                let end = ((i + 1) * bands.len() / col_count).min(bands.len());
                let slice = &bands[start..end];
                if slice.is_empty() {
                    0.0
                } else {
                    slice.iter().sum::<f32>() / slice.len() as f32
                }
            })
            .collect();

        // Monstercat smoothing (two passes)
        Self::monstercat_smooth(&mut target, 0.75);
        Self::monstercat_smooth(&mut target, 0.75);

        // Initialize state buffers on first call or column count change
        if self.bars.len() != col_count {
            self.bars = vec![0.0; col_count];
            self.velocity = vec![0.0; col_count];
            self.peaks = vec![0.0; col_count];
            self.peak_hold = vec![0; col_count];
        }

        // Gravity-based animation + peak tracking
        for i in 0..col_count {
            if target[i] >= self.bars[i] {
                // Rising: snap up quickly
                self.bars[i] = self.bars[i] * 0.2 + target[i] * 0.8;
                self.velocity[i] = 0.0;
            } else {
                // Falling: gravity acceleration
                self.velocity[i] += 0.004;
                self.bars[i] -= self.velocity[i];
                if self.bars[i] < target[i] {
                    self.bars[i] = target[i];
                    self.velocity[i] = 0.0;
                }
            }
            self.bars[i] = self.bars[i].clamp(0.0, 1.0);

            // Peak tracking with hold + slow gravity decay
            if self.bars[i] >= self.peaks[i] {
                self.peaks[i] = self.bars[i];
                self.peak_hold[i] = 25;
            } else if self.peak_hold[i] > 0 {
                self.peak_hold[i] -= 1;
            } else {
                // Slow gravity on the peak dot itself
                self.peaks[i] = (self.peaks[i] - 0.008).max(0.0);
            }
        }

        let buf = frame.buffer_mut();

        // Centered horizontal layout: col_count * 2 chars wide
        let total_used = col_count * 2;
        let offset_x = width.saturating_sub(total_used) / 2;

        for col in 0..col_count {
            let x = area.x + offset_x as u16 + (col * 2) as u16;
            if x + 1 >= area.x + area.width {
                break;
            }

            let amplitude = self.bars[col];
            // How many rows from the bottom should be lit
            let lit_rows = (amplitude * row_count as f32).round() as usize;
            // Peak row index from the bottom (0 = bottom)
            let peak_row_from_bottom = (self.peaks[col] * (row_count as f32 - 1.0)).round() as usize;

            for row in 0..row_count {
                // row 0 = top terminal row, row (row_count-1) = bottom terminal row
                let y = area.y + row as u16;
                if y >= area.y + area.height {
                    continue;
                }

                // Distance from bottom: 0 = bottom row, row_count-1 = top row
                let row_from_bottom = row_count - 1 - row;
                let is_peak = row_from_bottom == peak_row_from_bottom && self.peaks[col] > 0.02;
                let is_lit = row_from_bottom < lit_rows;

                let (dot_char, color) = if is_peak && !is_lit {
                    // Peak dot: accent color
                    ('●', theme.accent)
                } else if is_lit {
                    // Lit dot: brightness increases toward the top of the lit column
                    let height_fraction = if lit_rows > 0 {
                        row_from_bottom as f32 / lit_rows.max(1) as f32
                    } else {
                        0.0
                    };
                    let brightness = 0.45 + height_fraction * 0.55;
                    ('●', Self::scale_color(theme.secondary, brightness))
                } else {
                    // Dim / off dot
                    ('·', Self::scale_color(theme.dimmed, 0.25))
                };

                let cell = &mut buf[(x, y)];
                cell.set_char(dot_char);
                cell.set_fg(color);

                // The space after the dot
                let cell_space = &mut buf[(x + 1, y)];
                cell_space.set_char(' ');
            }
        }
    }
}
