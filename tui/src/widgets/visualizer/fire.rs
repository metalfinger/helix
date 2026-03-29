use super::VisualizerStyle;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Fire Spectrum: frequency bands rendered as flame columns.
/// Each column uses shade block characters (░▒▓█) with a warm fire palette.
/// No theme colors — fire uses its own hardcoded heat gradient.
pub struct Fire {
    /// Current displayed bar heights (gravity-smoothed)
    pub bars: Vec<f32>,
    /// Gravity velocity per bar
    pub velocity: Vec<f32>,
}

impl Fire {
    pub fn new() -> Self {
        Self {
            bars: Vec::new(),
            velocity: Vec::new(),
        }
    }

    /// Monstercat-style smoothing with configurable factor (0.7 for fire).
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

    /// Fire heat color based on vertical position within a bar.
    /// `heat` is 0.0 (coolest tip) to 1.0 (hottest base).
    fn fire_color(heat: f32) -> Color {
        if heat > 0.85 {
            // Base: bright white-yellow
            let t = (heat - 0.85) / 0.15;
            let r = lerp(255.0, 255.0, t) as u8;
            let g = lerp(160.0, 255.0, t) as u8;
            let b = lerp(40.0, 200.0, t) as u8;
            Color::Rgb(r, g, b)
        } else if heat > 0.55 {
            // Middle: orange
            let t = (heat - 0.55) / 0.30;
            let r = lerp(200.0, 255.0, t) as u8;
            let g = lerp(50.0, 160.0, t) as u8;
            let b = lerp(20.0, 40.0, t) as u8;
            Color::Rgb(r, g, b)
        } else if heat > 0.25 {
            // Upper: red
            let t = (heat - 0.25) / 0.30;
            let r = lerp(100.0, 200.0, t) as u8;
            let g = lerp(20.0, 50.0, t) as u8;
            let b = lerp(5.0, 20.0, t) as u8;
            Color::Rgb(r, g, b)
        } else {
            // Tips: dark red, nearly black
            let t = heat / 0.25;
            let r = lerp(10.0, 100.0, t) as u8;
            let g = lerp(2.0, 20.0, t) as u8;
            let b = lerp(1.0, 5.0, t) as u8;
            Color::Rgb(r, g, b)
        }
    }

    /// Deterministic pseudo-random flicker offset for a bar at a given tick.
    /// Returns a small signed float (-0.05 to +0.05) for tip displacement.
    fn flicker(tick: u64, bar_index: usize) -> f32 {
        let pseudo = (tick.wrapping_mul(7).wrapping_add(bar_index as u64 * 13)) % 5;
        // Map 0-4 → -0.04 to +0.04
        pseudo as f32 * 0.02 - 0.04
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.min(1.0).max(0.0)
}

impl VisualizerStyle for Fire {
    fn name(&self) -> &str {
        "Fire Spectrum"
    }

    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        bands: &[f32],
        _theme: &Theme,
        tick: u64,
    ) {
        if area.width < 4 || area.height < 2 || bands.is_empty() {
            return;
        }

        let width = area.width as usize;
        let height = area.height as usize;

        // Bar count: ~2 chars per bar with 1-char gap
        let bar_count = (width / 2).max(4).min(48);

        // Resample bands to bar_count
        let mut target = Self::resample(bands, bar_count);

        // Monstercat smoothing for fire (softer blend, factor 0.7)
        Self::monstercat_smooth(&mut target, 0.7);
        Self::monstercat_smooth(&mut target, 0.7);

        // Initialize buffers if needed
        if self.bars.len() != bar_count {
            self.bars = vec![0.0; bar_count];
            self.velocity = vec![0.0; bar_count];
        }

        // Apply gravity-based drop animation
        for i in 0..bar_count {
            let t = target[i];
            if t >= self.bars[i] {
                self.bars[i] = self.bars[i] * 0.2 + t * 0.8;
                self.velocity[i] = 0.0;
            } else {
                self.velocity[i] += 0.005;
                self.bars[i] -= self.velocity[i];
                if self.bars[i] < t {
                    self.bars[i] = t;
                    self.velocity[i] = 0.0;
                }
            }
            self.bars[i] = self.bars[i].max(0.0).min(1.0);
        }

        let buf = frame.buffer_mut();

        // Bar layout
        let bar_width = (width / bar_count).max(1);
        let gap = if bar_width >= 3 { 1 } else { 0 };
        let draw_width = bar_width - gap;
        let total_used = bar_width * bar_count;
        let offset_x = width.saturating_sub(total_used) / 2;

        // Shade characters: density increases downward (hotter = denser)
        // Index 0 = lightest (tip), index 3 = densest (base)
        const FLAME_CHARS: &[char] = &['░', '▒', '▓', '█'];

        for i in 0..bar_count {
            let x_start = area.x + offset_x as u16 + (i * bar_width) as u16;

            // Apply deterministic flicker to top 1-2 rows
            let flicker_offset = Self::flicker(tick, i);
            let effective_amp = (self.bars[i] + flicker_offset).max(0.0).min(1.0);

            let bar_h_float = effective_amp * height as f32;
            let bar_h = bar_h_float as usize;
            let frac = bar_h_float - bar_h as f32;

            for row in 0..height {
                // row 0 = bottom (hottest), row height-1 = top (coolest)
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
                        // Heat: 1.0 at bottom row, decreases toward top
                        let heat = row as f32 / height.max(1) as f32;

                        // Choose shade character based on heat — denser at base
                        // Bottom quarter → '█', next → '▓', then '▒', tips → '░'
                        let char_idx = if heat > 0.75 {
                            3 // █
                        } else if heat > 0.45 {
                            2 // ▓
                        } else if heat > 0.20 {
                            1 // ▒
                        } else {
                            0 // ░
                        };
                        let ch = FLAME_CHARS[char_idx];
                        let color = Self::fire_color(heat);

                        buf[(x, y)].set_char(ch);
                        buf[(x, y)].set_fg(color);
                    } else if row == bar_h && frac > 0.05 {
                        // Fractional tip — use lightest shade char with dark red tip color
                        let heat = row as f32 / height.max(1) as f32;
                        let color = Self::fire_color(heat * frac); // dim by fraction
                        buf[(x, y)].set_char('░');
                        buf[(x, y)].set_fg(color);
                    }
                }
            }
        }
    }
}
