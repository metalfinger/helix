use super::VisualizerStyle;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Winamp-style oscilloscope: a single waveform line rendered with braille
/// characters for sub-cell vertical resolution.
///
/// Each band contributes a sine wave at a different frequency; they sum to
/// produce the displayed waveform.  Smooth frame-to-frame lerp prevents
/// jarring jumps.
///
/// Braille 2×4 dot layout (Unicode block starting at U+2800):
///   col 0 (left):  row 0 → 0x01, row 1 → 0x02, row 2 → 0x04, row 3 → 0x40
///   col 1 (right): row 0 → 0x08, row 1 → 0x10, row 2 → 0x20, row 3 → 0x80
pub struct Scope {
    prev_wave: Vec<f32>,
}

impl Scope {
    pub fn new() -> Self {
        Self { prev_wave: Vec::new() }
    }

    fn rgb(color: Color) -> (f32, f32, f32) {
        match color {
            Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
            _ => (128.0, 128.0, 128.0),
        }
    }

    /// Scale an RGB color by a brightness multiplier (0.0–1.0+).
    fn scale_color(color: Color, brightness: f32) -> Color {
        let (r, g, b) = Self::rgb(color);
        Color::Rgb(
            (r * brightness).clamp(0.0, 255.0) as u8,
            (g * brightness).clamp(0.0, 255.0) as u8,
            (b * brightness).clamp(0.0, 255.0) as u8,
        )
    }

    /// Synthesize a waveform sample at position `x` (0.0–1.0) from frequency
    /// bands.  Each band i contributes a sine at frequency (i+1) with
    /// amplitude proportional to the band value.
    fn synthesize(bands: &[f32], x: f32) -> f32 {
        if bands.is_empty() {
            return 0.0;
        }
        let mut sum = 0.0f32;
        let mut total_amp = 0.0f32;
        for (i, &amp) in bands.iter().enumerate() {
            let freq = (i + 1) as f32;
            let phase = x * freq * std::f32::consts::TAU;
            sum += amp * phase.sin();
            total_amp += amp;
        }
        // Normalize so the result stays in [-1, 1]
        if total_amp > 0.0 { sum / total_amp } else { 0.0 }
    }

    /// Lerp element-wise; resizes `prev` to match `target` if needed.
    fn lerp_wave(prev: &mut Vec<f32>, target: &[f32], t: f32) {
        if prev.len() != target.len() {
            prev.resize(target.len(), 0.0);
        }
        for (p, &tgt) in prev.iter_mut().zip(target.iter()) {
            *p = *p + (tgt - *p) * t;
        }
    }
}

impl VisualizerStyle for Scope {
    fn name(&self) -> &str {
        "Scope"
    }

    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        bands: &[f32],
        theme: &Theme,
        tick: u64,
    ) {
        if area.width < 2 || area.height < 1 || bands.is_empty() {
            return;
        }

        let width = area.width as usize;
        let height = area.height as usize;

        // Each braille cell is 2 columns wide × 4 rows tall.
        // We have `width` terminal columns, each = 2 braille sub-columns.
        let num_samples = width * 2; // one sample per braille sub-column

        // Build raw waveform (time varies with tick for motion)
        let phase_offset = (tick as f32) * 0.04;
        let raw: Vec<f32> = (0..num_samples)
            .map(|i| {
                let x = i as f32 / num_samples as f32 + phase_offset;
                Self::synthesize(bands, x)
            })
            .collect();

        // Lerp toward raw (smooth transition, t=0.35 per frame)
        Self::lerp_wave(&mut self.prev_wave, &raw, 0.35);
        let wave = self.prev_wave.clone();

        let buf = frame.buffer_mut();

        // Draw dim center reference line
        let center_y = area.y + (height / 2) as u16;
        if center_y < area.y + area.height {
            for col in 0..width as u16 {
                let x = area.x + col;
                if x < area.x + area.width {
                    let cell = &mut buf[(x, center_y)];
                    // Only draw the reference if cell is currently blank
                    if cell.symbol() == " " {
                        cell.set_char('─');
                        cell.set_fg(Self::scale_color(theme.dimmed, 0.4));
                    }
                }
            }
        }

        // Braille dot offsets
        //   Left column:  row 0→0x01, row 1→0x02, row 2→0x04, row 3→0x40
        //   Right column: row 0→0x08, row 1→0x10, row 2→0x20, row 3→0x80
        let left_dots: [u32; 4] = [0x01, 0x02, 0x04, 0x40];
        let right_dots: [u32; 4] = [0x08, 0x10, 0x20, 0x80];

        // Total braille rows across the terminal rows
        let total_rows = height * 4;

        for col in 0..width {
            let x = area.x + col as u16;
            if x >= area.x + area.width {
                break;
            }

            // Two sub-columns per terminal column
            let left_sample = wave[col * 2];
            let right_sample = wave[col * 2 + 1];

            // Map [-1, 1] → [0, total_rows - 1]
            let map_row = |v: f32| -> usize {
                let normalized = (1.0 - (v.clamp(-1.0, 1.0) + 1.0) * 0.5) as f32;
                (normalized * (total_rows - 1) as f32).round() as usize
            };

            let left_row = map_row(left_sample);
            let right_row = map_row(right_sample);

            // Group into terminal rows (each = 4 braille rows)
            // We need to set one dot per column; group by which terminal cell they land in
            // and accumulate the braille pattern.
            struct Dot {
                term_row: usize,
                pattern: u32,
            }

            let dots = [
                Dot { term_row: left_row / 4, pattern: left_dots[left_row % 4] },
                Dot { term_row: right_row / 4, pattern: right_dots[right_row % 4] },
            ];

            // Merge patterns by terminal row
            let mut row_patterns = [0u32; 64]; // height won't exceed 64 in practice
            for dot in &dots {
                if dot.term_row < height {
                    row_patterns[dot.term_row] |= dot.pattern;
                }
            }

            // Amplitude for color brightness (average of both samples)
            let amplitude = (left_sample.abs() + right_sample.abs()) * 0.5;
            let brightness = 0.5 + amplitude * 0.5;

            for (row_idx, &pattern) in row_patterns.iter().enumerate().take(height) {
                if pattern == 0 {
                    continue;
                }
                let y = area.y + row_idx as u16;
                if y >= area.y + area.height {
                    continue;
                }
                let ch = char::from_u32(0x2800 | pattern).unwrap_or('·');
                let color = Self::scale_color(theme.secondary, brightness);
                let cell = &mut buf[(x, y)];
                cell.set_char(ch);
                cell.set_fg(color);
            }
        }
    }
}
