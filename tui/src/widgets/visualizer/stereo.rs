use super::VisualizerStyle;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Stereo Analyzer: mirrored bar display.
/// Upper half shows bars growing upward (left channel).
/// Lower half shows bars growing downward mirrored (right channel).
/// A thin center divider separates the two halves.
pub struct Stereo {
    /// Current displayed bar heights for upward (left) bars
    pub up_bars: Vec<f32>,
    /// Current displayed bar heights for downward (right) bars
    pub down_bars: Vec<f32>,
    /// Gravity velocity for up bars
    pub up_vel: Vec<f32>,
    /// Gravity velocity for down bars
    pub down_vel: Vec<f32>,
}

impl Stereo {
    pub fn new() -> Self {
        Self {
            up_bars: Vec::new(),
            down_bars: Vec::new(),
            up_vel: Vec::new(),
            down_vel: Vec::new(),
        }
    }

    /// Monstercat-style smoothing: forward and backward pass,
    /// each bar is at least `factor` fraction of its neighbor.
    fn monstercat_smooth(bars: &mut [f32], factor: f32) {
        let n = bars.len();
        if n < 3 {
            return;
        }
        for i in 1..n {
            let prev = bars[i - 1] * factor;
            if bars[i] < prev {
                bars[i] = prev;
            }
        }
        for i in (0..n - 1).rev() {
            let next = bars[i + 1] * factor;
            if bars[i] < next {
                bars[i] = next;
            }
        }
    }

    /// Resample `source` into `count` bars by averaging.
    fn resample(source: &[f32], count: usize) -> Vec<f32> {
        (0..count)
            .map(|i| {
                let start = i * source.len() / count;
                let end = ((i + 1) * source.len() / count).min(source.len());
                if end <= start {
                    return 0.0;
                }
                source[start..end].iter().sum::<f32>() / (end - start) as f32
            })
            .collect()
    }

    /// Apply gravity-based drop-off animation to a set of bars.
    fn apply_gravity(current: &mut [f32], velocity: &mut [f32], target: &[f32]) {
        for i in 0..current.len() {
            let t = target.get(i).copied().unwrap_or(0.0);
            if t >= current[i] {
                // Rising: snap up quickly
                current[i] = current[i] * 0.2 + t * 0.8;
                velocity[i] = 0.0;
            } else {
                // Falling: gravity acceleration
                velocity[i] += 0.005;
                current[i] -= velocity[i];
                if current[i] < t {
                    current[i] = t;
                    velocity[i] = 0.0;
                }
            }
            current[i] = current[i].max(0.0).min(1.0);
        }
    }

    /// Brightness gradient: brighter toward the center divider.
    /// `dist_from_center` is 0.0 at center, 1.0 at the far edge.
    fn gradient_color(dist_from_center: f32, theme: &Theme) -> Color {
        let (r, g, b) = match theme.secondary {
            Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
            _ => (128.0, 128.0, 128.0),
        };
        // Brighter near center (dist = 0), dimmer toward edges (dist = 1)
        let brightness = 1.0 - dist_from_center * 0.65;
        Color::Rgb(
            (r * brightness).min(255.0) as u8,
            (g * brightness).min(255.0) as u8,
            (b * brightness).min(255.0) as u8,
        )
    }
}

impl VisualizerStyle for Stereo {
    fn name(&self) -> &str {
        "Stereo Analyzer"
    }

    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        bands: &[f32],
        theme: &Theme,
        _tick: u64,
    ) {
        if area.width < 4 || area.height < 4 || bands.is_empty() {
            return;
        }

        let width = area.width as usize;
        let height = area.height as usize;

        // Bar count: ~2 chars per bar with 1-char gap
        let bar_count = (width / 2).max(4).min(48);

        // Split bands in half: first half → up, second half → down
        let mid = bands.len() / 2;
        let up_source = if mid > 0 { &bands[..mid] } else { bands };
        let down_source = if bands.len() > mid { &bands[mid..] } else { bands };

        let mut up_target = Self::resample(up_source, bar_count);
        let mut down_target = Self::resample(down_source, bar_count);

        // Monstercat smoothing (two passes)
        Self::monstercat_smooth(&mut up_target, 0.75);
        Self::monstercat_smooth(&mut up_target, 0.75);
        Self::monstercat_smooth(&mut down_target, 0.75);
        Self::monstercat_smooth(&mut down_target, 0.75);

        // Initialize buffers if bar count changed
        if self.up_bars.len() != bar_count {
            self.up_bars = vec![0.0; bar_count];
            self.down_bars = vec![0.0; bar_count];
            self.up_vel = vec![0.0; bar_count];
            self.down_vel = vec![0.0; bar_count];
        }

        // Apply gravity animation
        Self::apply_gravity(&mut self.up_bars, &mut self.up_vel, &up_target);
        Self::apply_gravity(&mut self.down_bars, &mut self.down_vel, &down_target);

        let buf = frame.buffer_mut();

        // Vertical layout: upper half for up-bars, center divider, lower half for down-bars
        let center_y = area.y + (height / 2) as u16;
        let upper_h = height / 2; // rows available above center
        let lower_h = height - upper_h - 1; // rows available below center (subtract 1 for divider)

        // Draw center divider
        {
            let (r, g, b) = match theme.dimmed {
                Color::Rgb(r, g, b) => (r, g, b),
                _ => (80, 80, 80),
            };
            for x in area.x..area.x + area.width {
                buf[(x, center_y)].set_char('─');
                buf[(x, center_y)].set_fg(Color::Rgb(r, g, b));
            }
        }

        // Bar layout
        let bar_width = (width / bar_count).max(1);
        let gap = if bar_width >= 3 { 1 } else { 0 };
        let draw_width = bar_width - gap;
        let total_used = bar_width * bar_count;
        let offset_x = width.saturating_sub(total_used) / 2;

        const UP_CHARS: &[char] = &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

        for i in 0..bar_count {
            let x_start = area.x + offset_x as u16 + (i * bar_width) as u16;

            // --- Upper half: bars grow upward from center ---
            {
                let amplitude = self.up_bars[i];
                let bar_h_float = amplitude * upper_h as f32;
                let bar_h = bar_h_float as usize;
                let frac = bar_h_float - bar_h as f32;

                for row in 0..upper_h {
                    // row 0 = just above center, increases upward
                    let y = center_y - 1 - row as u16;
                    if y < area.y {
                        break;
                    }
                    let dist_from_center = row as f32 / upper_h.max(1) as f32;
                    let color = Self::gradient_color(dist_from_center, theme);

                    for bw in 0..draw_width {
                        let x = x_start + bw as u16;
                        if x >= area.x + area.width {
                            continue;
                        }
                        if row < bar_h {
                            buf[(x, y)].set_char('█');
                            buf[(x, y)].set_fg(color);
                        } else if row == bar_h && frac > 0.05 {
                            let idx = (frac * 8.0).round() as usize;
                            let ch = UP_CHARS[idx.min(UP_CHARS.len() - 1)];
                            buf[(x, y)].set_char(ch);
                            buf[(x, y)].set_fg(color);
                        }
                    }
                }
            }

            // --- Lower half: bars grow downward from center ---
            {
                let amplitude = self.down_bars[i];
                let bar_h_float = amplitude * lower_h as f32;
                let bar_h = bar_h_float as usize;
                let frac = bar_h_float - bar_h as f32;

                for row in 0..lower_h {
                    // row 0 = just below center, increases downward
                    let y = center_y + 1 + row as u16;
                    if y >= area.y + area.height {
                        break;
                    }
                    let dist_from_center = row as f32 / lower_h.max(1) as f32;
                    let color = Self::gradient_color(dist_from_center, theme);

                    for bw in 0..draw_width {
                        let x = x_start + bw as u16;
                        if x >= area.x + area.width {
                            continue;
                        }
                        if row < bar_h {
                            buf[(x, y)].set_char('█');
                            buf[(x, y)].set_fg(color);
                        } else if row == bar_h && frac > 0.05 {
                            // For downward bars, use inverted fractional character
                            // '▔' represents a thin top — gives sense of partial fill at edge
                            let idx = (frac * 8.0).round() as usize;
                            let ch = UP_CHARS[idx.min(UP_CHARS.len() - 1)];
                            buf[(x, y)].set_char(ch);
                            buf[(x, y)].set_fg(color);
                        }
                    }
                }
            }
        }
    }
}
