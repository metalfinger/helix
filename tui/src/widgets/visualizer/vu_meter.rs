use super::VisualizerStyle;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

const FILLED: char = '█';
const EMPTY: char = '░';
const PEAK: char = '▎';

/// Classic analog-style dual-channel VU meter.
///
/// Low-frequency bands drive the Left channel; high-frequency bands drive
/// the Right channel.  Each channel has:
///   - A horizontal level bar (smooth rise, gravity-based fall)
///   - A peak-needle marker that overshoots and falls back with gravity
///   - dB-scale tick marks along the bottom of the meter area
///
/// Colors:
///   - Fill:  theme.secondary
///   - Empty: theme.dimmed
///   - Peak:  theme.accent
pub struct VuMeter {
    left_val: f32,
    right_val: f32,
    left_peak: f32,
    right_peak: f32,
    left_vel: f32,
    right_vel: f32,
}

impl VuMeter {
    pub fn new() -> Self {
        Self {
            left_val: 0.0,
            right_val: 0.0,
            left_peak: 0.0,
            right_peak: 0.0,
            left_vel: 0.0,
            right_vel: 0.0,
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

    /// Compute channel level as RMS-ish average of the given band slice.
    fn band_level(bands: &[f32]) -> f32 {
        if bands.is_empty() {
            return 0.0;
        }
        let sum: f32 = bands.iter().map(|v| v * v).sum();
        (sum / bands.len() as f32).sqrt().min(1.0)
    }

    /// Update one channel: fast attack, gravity fall, peak tracking.
    fn update_channel(val: &mut f32, vel: &mut f32, peak: &mut f32, target: f32) {
        if target >= *val {
            // Fast attack
            *val = *val * 0.15 + target * 0.85;
            *vel = 0.0;
        } else {
            // Gravity fall
            *vel += 0.006;
            *val -= *vel;
            if *val < target {
                *val = target;
                *vel = 0.0;
            }
        }
        *val = val.clamp(0.0, 1.0);

        // Peak: swing forward, fall back with gravity
        if *val >= *peak {
            *peak = *val;
        } else {
            // Very slow fall for the peak needle
            *peak = (*peak - 0.003).max(*val).max(0.0);
        }
        *peak = peak.clamp(0.0, 1.0);
    }

    /// Draw one horizontal VU bar at terminal row `y`.
    fn draw_bar(
        frame: &mut Frame,
        area: Rect,
        y: u16,
        label: char,
        value: f32,
        peak: f32,
        bar_width: u16,
        theme: &Theme,
    ) {
        if y >= area.y + area.height {
            return;
        }
        let buf = frame.buffer_mut();

        // Label: "L" or "R"
        let lx = area.x;
        if lx < area.x + area.width {
            buf[(lx, y)].set_char(label);
            buf[(lx, y)].set_fg(Self::scale_color(theme.secondary, 0.8));
        }

        // Space after label
        let bar_start = area.x + 2; // 1 label + 1 space
        let bar_end = bar_start + bar_width;
        if bar_start >= area.x + area.width {
            return;
        }
        let bar_end = bar_end.min(area.x + area.width);
        let actual_width = (bar_end - bar_start) as usize;

        let filled_cells = (value * actual_width as f32).round() as usize;
        let peak_cell = (peak * actual_width as f32).round() as usize;

        // Color gets brighter as we approach the right (louder = warmer)
        for i in 0..actual_width {
            let x = bar_start + i as u16;
            if x >= bar_end {
                break;
            }

            if i == peak_cell.min(actual_width.saturating_sub(1)) && peak > 0.01 {
                // Peak needle marker
                buf[(x, y)].set_char(PEAK);
                buf[(x, y)].set_fg(theme.accent);
            } else if i < filled_cells {
                // Filled portion — brightness gradient left→right
                let pos = i as f32 / actual_width as f32;
                let brightness = 0.5 + pos * 0.5;
                buf[(x, y)].set_char(FILLED);
                buf[(x, y)].set_fg(Self::scale_color(theme.secondary, brightness));
            } else {
                // Empty portion
                buf[(x, y)].set_char(EMPTY);
                buf[(x, y)].set_fg(Self::scale_color(theme.dimmed, 0.5));
            }
        }
    }

    /// Draw dB scale markers below the bars.
    /// Markers: -20, -10, -5, 0, +3 (mapped linearly to bar width)
    fn draw_scale(frame: &mut Frame, area: Rect, y: u16, bar_start: u16, bar_width: u16, theme: &Theme) {
        if y >= area.y + area.height {
            return;
        }
        let buf = frame.buffer_mut();

        // dB scale: map dB value to 0–1 position using simplified log scale
        // We treat 0 dB as 1.0 and -20 dB as 0.0; +3 dB clips above 1.0
        // Position formula: pos = (db + 20) / 23   (+3 maps to 1.0 at +3+20=23)
        let markers: &[(&str, f32)] = &[
            ("-20", (-20.0_f32 + 20.0) / 23.0),
            ("-10", (-10.0_f32 + 20.0) / 23.0),
            (" -5", (-5.0_f32 + 20.0) / 23.0),
            ("  0", (0.0_f32 + 20.0) / 23.0),
            (" +3", (3.0_f32 + 20.0) / 23.0),
        ];

        let color = Self::scale_color(theme.dimmed, 0.7);

        for (label, pos) in markers {
            let col = bar_start + (pos * bar_width as f32).round() as u16;
            // Draw tick
            if col < area.x + area.width {
                buf[(col, y)].set_char('▲');
                buf[(col, y)].set_fg(color);
            }
            // Draw label (3 chars) centered on tick, shifted slightly left
            let label_start = col.saturating_sub(1);
            for (ci, ch) in label.chars().enumerate() {
                let lx = label_start + ci as u16;
                if lx < area.x + area.width && y + 1 < area.y + area.height {
                    buf[(lx, y + 1)].set_char(ch);
                    buf[(lx, y + 1)].set_fg(color);
                }
            }
        }
    }
}

impl VisualizerStyle for VuMeter {
    fn name(&self) -> &str {
        "VU Meter"
    }

    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        bands: &[f32],
        theme: &Theme,
        _tick: u64,
    ) {
        if area.width < 6 || area.height < 2 || bands.is_empty() {
            return;
        }

        // Split bands: odd-indexed → left, even-indexed → right
        // This gives balanced L/R since we don't have true stereo data
        let left_bands: Vec<f32> = bands.iter().step_by(2).copied().collect();
        let right_bands: Vec<f32> = bands.iter().skip(1).step_by(2).copied().collect();
        let left_target = Self::band_level(&left_bands);
        let right_target = Self::band_level(&right_bands);

        Self::update_channel(&mut self.left_val, &mut self.left_vel, &mut self.left_peak, left_target);
        Self::update_channel(&mut self.right_val, &mut self.right_vel, &mut self.right_peak, right_target);

        let height = area.height as usize;

        // Layout: center the two bars vertically.
        // We need 2 bar rows + 1 scale row (tick) + 1 scale row (label) = 4 rows min
        // If height < 4, skip scale; if height < 2, skip entirely.
        let (left_y, right_y, scale_y) = if height >= 4 {
            let top = area.y + (height as u16 / 2).saturating_sub(1);
            (top, top + 1, Some(top + 2))
        } else if height >= 2 {
            let top = area.y;
            (top, top + 1, None)
        } else {
            (area.y, area.y, None)
        };

        // Bar width: total width minus 2 (label + space)
        let bar_width = area.width.saturating_sub(2);

        Self::draw_bar(frame, area, left_y, 'L', self.left_val, self.left_peak, bar_width, theme);
        Self::draw_bar(frame, area, right_y, 'R', self.right_val, self.right_peak, bar_width, theme);

        if let Some(sy) = scale_y {
            let bar_start = area.x + 2;
            Self::draw_scale(frame, area, sy, bar_start, bar_width, theme);
        }
    }
}
