use super::VisualizerStyle;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// CAVA-inspired bar spectrum with:
/// - Monstercat-style smoothing (adjacent bars blend into smooth curves)
/// - Gravity-based drop-off (bars fall naturally, not instantly)
/// - Noise reduction (filters out low-level noise floor)
/// - Gradient coloring (bottom dim → top bright, hue shifts across bars)
/// - Peak hold indicators with slow decay
const BAR_CHARS: &[char] = &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub struct BarSpectrum {
    /// Current displayed bar heights (with gravity applied)
    bars: Vec<f32>,
    /// Velocity for gravity drop-off per bar
    velocity: Vec<f32>,
    /// Peak positions
    peaks: Vec<f32>,
    /// Peak hold countdown per bar
    peak_hold: Vec<u8>,
    /// Previous raw input for noise reduction
    prev_input: Vec<f32>,
}

impl BarSpectrum {
    pub fn new() -> Self {
        Self {
            bars: Vec::new(),
            velocity: Vec::new(),
            peaks: Vec::new(),
            peak_hold: Vec::new(),
            prev_input: Vec::new(),
        }
    }

    /// Monstercat-style smoothing: each bar pulls its neighbors toward it,
    /// creating smooth flowing curves instead of jagged bars.
    fn monstercat_smooth(bars: &mut [f32], strength: f32) {
        let n = bars.len();
        if n < 3 {
            return;
        }
        // Forward pass: each bar is at least `strength` fraction of its left neighbor
        for i in 1..n {
            let prev = bars[i - 1] * strength;
            if bars[i] < prev {
                bars[i] = prev;
            }
        }
        // Backward pass: same from right
        for i in (0..n - 1).rev() {
            let next = bars[i + 1] * strength;
            if bars[i] < next {
                bars[i] = next;
            }
        }
    }

    /// Noise reduction: suppress values below a threshold and smooth with previous frame
    fn noise_reduce(current: &[f32], prev: &[f32], threshold: f32) -> Vec<f32> {
        current
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let v = if v < threshold { 0.0 } else { v };
                let p = prev.get(i).copied().unwrap_or(0.0);
                // Integral smoothing with previous frame
                v * 0.65 + p * 0.35
            })
            .collect()
    }

    fn gradient_color(_bar_pos: f32, height_pos: f32, theme: &Theme) -> Color {
        // Single color, vertical brightness gradient: dim at bottom → bright at top
        let (r, g, b) = Self::rgb(theme.secondary);
        let brightness = 0.3 + height_pos * 0.7;
        Color::Rgb(
            (r * brightness).min(255.0) as u8,
            (g * brightness).min(255.0) as u8,
            (b * brightness).min(255.0) as u8,
        )
    }

    fn rgb(color: Color) -> (f32, f32, f32) {
        match color {
            Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
            _ => (128.0, 128.0, 128.0),
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

impl VisualizerStyle for BarSpectrum {
    fn name(&self) -> &str {
        "Bar Spectrum"
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

        // Target bar count: ~2 chars wide with 1 gap
        let bar_count = (width / 2).max(4).min(48);

        // Resample bands to bar count
        let mut target: Vec<f32> = (0..bar_count)
            .map(|i| {
                let start = i * bands.len() / bar_count;
                let end = ((i + 1) * bands.len() / bar_count).min(bands.len());
                let sum: f32 = bands[start..end].iter().sum();
                sum / (end - start).max(1) as f32
            })
            .collect();

        // Noise reduction
        target = Self::noise_reduce(&target, &self.prev_input, 0.02);
        self.prev_input = target.clone();

        // Monstercat smoothing (run twice for stronger effect)
        Self::monstercat_smooth(&mut target, 0.75);
        Self::monstercat_smooth(&mut target, 0.75);

        // Initialize buffers if needed
        if self.bars.len() != bar_count {
            self.bars = vec![0.0; bar_count];
            self.velocity = vec![0.0; bar_count];
            self.peaks = vec![0.0; bar_count];
            self.peak_hold = vec![0; bar_count];
        }

        // Apply gravity-based animation
        for i in 0..bar_count {
            if target[i] >= self.bars[i] {
                // Rising: snap up quickly
                self.bars[i] = self.bars[i] * 0.2 + target[i] * 0.8;
                self.velocity[i] = 0.0;
            } else {
                // Falling: gravity drop-off
                self.velocity[i] += 0.005; // gravity acceleration
                self.bars[i] -= self.velocity[i];
                if self.bars[i] < target[i] {
                    self.bars[i] = target[i];
                    self.velocity[i] = 0.0;
                }
            }
            self.bars[i] = self.bars[i].max(0.0).min(1.0);

            // Peak tracking
            if self.bars[i] >= self.peaks[i] {
                self.peaks[i] = self.bars[i];
                self.peak_hold[i] = 20;
            } else if self.peak_hold[i] > 0 {
                self.peak_hold[i] -= 1;
            } else {
                self.peaks[i] = (self.peaks[i] - 0.01).max(0.0);
            }
        }

        let buf = frame.buffer_mut();

        // Bar layout
        let bar_width = (width / bar_count).max(1);
        let gap = if bar_width >= 3 { 1 } else { 0 };
        let draw_width = bar_width - gap;
        let total_used = bar_width * bar_count;
        let offset_x = width.saturating_sub(total_used) / 2;

        for (i, &amplitude) in self.bars.iter().enumerate() {
            let bar_pos = i as f32 / bar_count as f32;
            let x_start = area.x + offset_x as u16 + (i * bar_width) as u16;

            let bar_h_float = amplitude * height as f32;
            let bar_h = bar_h_float as usize;

            for row in 0..height {
                let y = area.y + (height - 1 - row) as u16;
                if y >= area.y + area.height {
                    continue;
                }

                for bw in 0..draw_width {
                    let x = x_start + bw as u16;
                    if x >= area.x + area.width {
                        continue;
                    }

                    if row < bar_h {
                        // Filled bar with gradient
                        let height_pos = row as f32 / height as f32;
                        let color = Self::gradient_color(bar_pos, height_pos, theme);
                        buf[(x, y)].set_char('█');
                        buf[(x, y)].set_fg(color);
                    } else if row == bar_h {
                        // Fractional top — sub-character precision
                        let frac = bar_h_float - bar_h as f32;
                        if frac > 0.05 {
                            let idx = (frac * 8.0).round() as usize;
                            let ch = BAR_CHARS[idx.min(BAR_CHARS.len() - 1)];
                            let height_pos = row as f32 / height as f32;
                            let color = Self::gradient_color(bar_pos, height_pos, theme);
                            buf[(x, y)].set_char(ch);
                            buf[(x, y)].set_fg(color);
                        }
                    }
                }
            }

            // Peak indicator line
            let peak_row = (self.peaks[i] * height as f32) as usize;
            if peak_row > 0 && peak_row < height {
                let y = area.y + (height - 1 - peak_row) as u16;
                if y >= area.y && y < area.y + area.height {
                    for bw in 0..draw_width {
                        let x = x_start + bw as u16;
                        if x < area.x + area.width {
                            buf[(x, y)].set_char('▔');
                            buf[(x, y)].set_fg(theme.text);
                        }
                    }
                }
            }
        }
    }
}
