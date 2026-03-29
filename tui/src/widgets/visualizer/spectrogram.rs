use super::VisualizerStyle;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Time-frequency waterfall display.
///
/// Each row is a snapshot of the spectrum at a point in time. New data is pushed
/// to the top each tick and older rows scroll downward, creating a waterfall effect.
/// Cell intensity is encoded via block characters: ' ' ░ ▒ ▓ █
const INTENSITY_CHARS: &[char] = &[' ', '░', '▒', '▓', '█'];

pub struct Spectrogram {
    /// Ring buffer of spectrum snapshots — newest at index 0, oldest at the end.
    history: Vec<Vec<f32>>,
    /// Whether the buffers have been sized to the current area.
    initialized: bool,
}

impl Spectrogram {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            initialized: false,
        }
    }

    /// Resample `src` to exactly `target_len` bins via linear interpolation.
    fn resample(src: &[f32], target_len: usize) -> Vec<f32> {
        if src.is_empty() || target_len == 0 {
            return vec![0.0; target_len];
        }
        if src.len() == target_len {
            return src.to_vec();
        }
        (0..target_len)
            .map(|i| {
                let t = i as f32 / (target_len - 1).max(1) as f32;
                let src_pos = t * (src.len() - 1) as f32;
                let lo = src_pos.floor() as usize;
                let hi = (lo + 1).min(src.len() - 1);
                let frac = src_pos - lo as f32;
                src[lo] * (1.0 - frac) + src[hi] * frac
            })
            .collect()
    }

    fn rgb(color: Color) -> (f32, f32, f32) {
        match color {
            Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
            _ => (128.0, 128.0, 128.0),
        }
    }

    /// Map an amplitude (0.0–1.0) to a block character.
    fn intensity_char(amplitude: f32) -> char {
        let idx = (amplitude * (INTENSITY_CHARS.len() - 1) as f32).round() as usize;
        INTENSITY_CHARS[idx.min(INTENSITY_CHARS.len() - 1)]
    }

    /// Tint theme.secondary by a brightness factor in 0.0–1.0.
    fn amplitude_color(amplitude: f32, theme: &Theme) -> Color {
        let (r, g, b) = Self::rgb(theme.secondary);
        // Keep a minimum floor so the grid never goes fully black
        let brightness = 0.15 + amplitude * 0.85;
        Color::Rgb(
            (r * brightness).min(255.0) as u8,
            (g * brightness).min(255.0) as u8,
            (b * brightness).min(255.0) as u8,
        )
    }
}

impl VisualizerStyle for Spectrogram {
    fn name(&self) -> &str {
        "Spectrogram"
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

        let w = area.width as usize;
        let h = area.height as usize;

        // Re-initialize history dimensions when area changes
        if !self.initialized || self.history.first().map_or(true, |r| r.len() != w) {
            self.history = vec![vec![0.0; w]; h];
            self.initialized = true;
        }

        // Boost higher bands so they're visible (treble is naturally quieter)
        let boosted: Vec<f32> = bands.iter().enumerate().map(|(i, &v)| {
            let boost = 1.0 + (i as f32 / bands.len() as f32) * 3.0;
            (v * boost).min(1.0)
        }).collect();

        // Build a new snapshot resampled to the current width
        let snapshot = Self::resample(&boosted, w);

        // Prepend new snapshot and discard oldest if we exceed height
        self.history.insert(0, snapshot);
        if self.history.len() > h {
            self.history.pop();
        }

        let buf = frame.buffer_mut();

        for (row_idx, row) in self.history.iter().enumerate() {
            let y = area.y + row_idx as u16;
            if y >= area.y + area.height {
                break;
            }
            for (col_idx, &amp) in row.iter().enumerate() {
                let x = area.x + col_idx as u16;
                if x >= area.x + area.width {
                    break;
                }
                let amp = amp.max(0.0).min(1.0);
                let ch = Self::intensity_char(amp);
                let color = Self::amplitude_color(amp, theme);
                buf[(x, y)].set_char(ch);
                buf[(x, y)].set_fg(color);
            }
        }
    }
}
