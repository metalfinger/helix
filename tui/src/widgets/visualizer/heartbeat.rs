use super::VisualizerStyle;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Hospital EKG / heart-monitor visualizer.
///
/// Displays a scrolling flatline that erupts into a sharp EKG spike whenever
/// significant audio energy is detected.  Rendered with braille characters for
/// smooth sub-cell vertical resolution (same technique as Scope).
///
/// Braille 2×4 dot layout (Unicode block starting at U+2800):
///   col 0 (left):  row 0 → 0x01, row 1 → 0x02, row 2 → 0x04, row 3 → 0x40
///   col 1 (right): row 0 → 0x08, row 1 → 0x10, row 2 → 0x20, row 3 → 0x80
///
/// The EKG spike pattern (normalized −1.0 … +1.0):
///   [0.0, 0.3, 0.8, −0.5, 0.2, 0.0, 0.0, 0.0]
/// is injected into `spike_queue` on beat detection; one sample drains per tick.
pub struct Heartbeat {
    /// Circular scrolling buffer of y-values (−1.0 … +1.0).
    /// Index 0 = leftmost (oldest) sample, last index = newest.
    buffer: Vec<f32>,
    /// Energy level from the previous frame, used for beat detection.
    prev_energy: f32,
    /// Queue of upcoming spike samples to inject on the right side.
    spike_queue: Vec<f32>,
}

/// EKG spike shape: sharp up, sharp down, small positive bump, then back to flat.
const SPIKE_PATTERN: &[f32] = &[0.0, 0.3, 0.8, -0.5, 0.2, 0.0, 0.0, 0.0];

/// Energy threshold below which beats are ignored (noise floor).
const BEAT_THRESHOLD: f32 = 0.15;
/// Beat is triggered when energy > prev_energy * this factor.
const BEAT_SENSITIVITY: f32 = 1.3;

impl Heartbeat {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            prev_energy: 0.0,
            spike_queue: Vec::new(),
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

    /// Compute RMS energy from the band amplitudes.
    fn compute_energy(bands: &[f32]) -> f32 {
        if bands.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = bands.iter().map(|v| v * v).sum();
        (sum_sq / bands.len() as f32).sqrt()
    }

    /// Shift the buffer left by one and append `new_val` on the right.
    fn scroll_buffer(buf: &mut Vec<f32>, new_val: f32) {
        if buf.is_empty() {
            return;
        }
        buf.copy_within(1.., 0);
        let last = buf.len() - 1;
        buf[last] = new_val;
    }
}

impl VisualizerStyle for Heartbeat {
    fn name(&self) -> &str {
        "Heartbeat"
    }

    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        bands: &[f32],
        theme: &Theme,
        _tick: u64,
    ) {
        if area.width < 2 || area.height < 1 {
            return;
        }

        let width = area.width as usize;
        let height = area.height as usize;

        // One braille cell = 2 sub-columns wide; buffer length = width * 2 sub-columns.
        let num_samples = width * 2;

        // Grow / shrink buffer to match terminal width
        if self.buffer.len() != num_samples {
            self.buffer.resize(num_samples, 0.0);
        }

        // Beat detection
        let energy = Self::compute_energy(bands);
        let beat = energy > BEAT_THRESHOLD && energy > self.prev_energy * BEAT_SENSITIVITY;
        self.prev_energy = energy;

        if beat && self.spike_queue.is_empty() {
            // Enqueue the EKG spike (scale amplitude by energy for punch)
            let scale = (energy * 1.5).min(1.0);
            for &s in SPIKE_PATTERN {
                self.spike_queue.push(s * scale);
            }
        }

        // Determine new sample to append
        let new_sample = if !self.spike_queue.is_empty() {
            self.spike_queue.remove(0)
        } else {
            0.0 // flatline
        };

        Self::scroll_buffer(&mut self.buffer, new_sample);

        // Find the sample closest to the peak for accent coloring
        let peak_sub_col = self.buffer
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        // Braille dot bitmasks
        let left_dots: [u32; 4] = [0x01, 0x02, 0x04, 0x40];
        let right_dots: [u32; 4] = [0x08, 0x10, 0x20, 0x80];

        let buf = frame.buffer_mut();
        let total_rows = height * 4; // braille sub-rows across terminal height

        // Draw dim grid: horizontal center line and vertical markers
        let center_term_row = (height / 2) as u16;
        for col in 0..width as u16 {
            let x = area.x + col;
            if x >= area.x + area.width {
                break;
            }
            let y = area.y + center_term_row;
            if y < area.y + area.height {
                let cell = &mut buf[(x, y)];
                if cell.symbol() == " " {
                    cell.set_char('─');
                    cell.set_fg(Self::scale_color(theme.dimmed, 0.3));
                }
            }
            // Vertical tick marks every ~20 terminal columns
            if col % 20 == 0 {
                for row in 0..height as u16 {
                    let y = area.y + row;
                    if y < area.y + area.height {
                        let cell = &mut buf[(x, y)];
                        if cell.symbol() == " " {
                            cell.set_char('│');
                            cell.set_fg(Self::scale_color(theme.dimmed, 0.2));
                        }
                    }
                }
            }
        }

        // Render the braille waveform
        for col in 0..width {
            let x = area.x + col as u16;
            if x >= area.x + area.width {
                break;
            }

            let left_sub = col * 2;
            let right_sub = col * 2 + 1;

            // Map sample value (−1.0 … +1.0) to braille sub-row index (0 = top).
            // Centre = total_rows / 2; positive values go upward.
            let map_to_sub_row = |v: f32| -> usize {
                let center = (total_rows as f32 - 1.0) * 0.5;
                let offset = v.clamp(-1.0, 1.0) * center * 0.9;
                (center - offset).round().clamp(0.0, (total_rows - 1) as f32) as usize
            };

            let left_sub_row = map_to_sub_row(self.buffer[left_sub]);
            let right_sub_row = map_to_sub_row(self.buffer[right_sub]);

            let mut row_patterns = [0u32; 64];
            let left_term = left_sub_row / 4;
            let right_term = right_sub_row / 4;
            if left_term < height {
                row_patterns[left_term] |= left_dots[left_sub_row % 4];
            }
            if right_term < height {
                row_patterns[right_term] |= right_dots[right_sub_row % 4];
            }

            // Choose color: accent at peak column, secondary elsewhere
            let is_peak_col = left_sub == peak_sub_col || right_sub == peak_sub_col;
            let max_abs = self.buffer[left_sub].abs().max(self.buffer[right_sub].abs());
            let color = if is_peak_col && max_abs > 0.3 {
                theme.accent
            } else {
                let brightness = 0.55 + max_abs * 0.45;
                Self::scale_color(theme.secondary, brightness)
            };

            for (row_idx, &pattern) in row_patterns.iter().enumerate().take(height) {
                if pattern == 0 {
                    continue;
                }
                let y = area.y + row_idx as u16;
                if y >= area.y + area.height {
                    continue;
                }
                let ch = char::from_u32(0x2800 | pattern).unwrap_or('·');
                let cell = &mut buf[(x, y)];
                cell.set_char(ch);
                cell.set_fg(color);
            }
        }
    }
}
