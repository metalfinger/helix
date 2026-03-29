use super::VisualizerStyle;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

const DROP_CHARS: [char; 8] = [
    '░', '▒', '▓', '█', '▄', '▀', '▌', '▐',
];

struct Drop {
    y: f32,
    speed: f32,
    len: usize,
    chars: Vec<char>,
}

/// Rainfall Spectrum: matrix-rain style visualization where each column
/// corresponds to a frequency band. Bass columns rain heavy; treble columns
/// rain sparse. Drop density, speed, and brightness track band amplitude.
pub struct Rainfall {
    drops: Vec<Vec<Drop>>,
    initialized: bool,
}

impl Rainfall {
    pub fn new() -> Self {
        Self {
            drops: Vec::new(),
            initialized: false,
        }
    }

    /// Deterministic pseudo-random value in [0, 1) based on tick, column, and row.
    fn pseudo_rand(tick: u64, col: usize, row: u64) -> f32 {
        let v = tick
            .wrapping_mul(7)
            .wrapping_add(col as u64 * 13)
            .wrapping_add(row * 31);
        (v % 1000) as f32 / 1000.0
    }

    /// Pick a random character for a drop position.
    fn rand_char(tick: u64, col: usize, row: u64) -> char {
        let n = DROP_CHARS.len() as u64;
        let v = tick
            .wrapping_mul(7)
            .wrapping_add(col as u64 * 13)
            .wrapping_add(row * 31);
        DROP_CHARS[(v % n) as usize]
    }

    /// Resample `source` into `count` values by averaging.
    fn resample(source: &[f32], count: usize) -> Vec<f32> {
        if source.is_empty() || count == 0 {
            return vec![0.0; count];
        }
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

    fn rgb(color: Color) -> (f32, f32, f32) {
        match color {
            Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
            _ => (128.0, 128.0, 128.0),
        }
    }
}

impl VisualizerStyle for Rainfall {
    fn name(&self) -> &str {
        "Rainfall"
    }

    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        bands: &[f32],
        theme: &Theme,
        tick: u64,
    ) {
        if area.width < 2 || area.height < 2 || bands.is_empty() {
            return;
        }

        let w = area.width as usize;
        let h = area.height as usize;

        // Initialize column drop storage if needed or if width changed.
        if !self.initialized || self.drops.len() != w {
            self.drops = (0..w).map(|_| Vec::new()).collect();
            self.initialized = true;
        }

        // Resample bands to match terminal width.
        let amplitudes = Self::resample(bands, w);

        // --- Update phase ---
        for (col, col_drops) in self.drops.iter_mut().enumerate() {
            let amp = amplitudes[col].clamp(0.0, 1.0);

            // Move existing drops downward.
            for drop in col_drops.iter_mut() {
                drop.y += drop.speed;
            }

            // Remove drops that have scrolled fully off the bottom.
            col_drops.retain(|d| (d.y - d.len as f32) < (h as f32));

            // Spawn new drops at the top based on amplitude.
            // Spawn probability = amplitude * 0.5 per tick.
            let spawn_prob = amp * 0.5;
            let rand_val = Self::pseudo_rand(tick, col, 999);
            if rand_val < spawn_prob {
                // Drop length 3–8
                let len_seed = Self::pseudo_rand(tick, col, 777);
                let len = 3 + (len_seed * 6.0) as usize; // 3..=8

                // Speed 0.3–1.5, scales with amplitude
                let speed_seed = Self::pseudo_rand(tick, col, 555);
                let speed = 0.3 + amp * 0.8 + speed_seed * 0.4;

                // Build the character array for this drop once.
                let chars: Vec<char> = (0..len)
                    .map(|i| Self::rand_char(tick, col, i as u64))
                    .collect();

                col_drops.push(Drop {
                    y: -(len as f32), // start fully above the top
                    speed,
                    len,
                    chars,
                });
            }
        }

        // --- Render phase ---
        let buf = frame.buffer_mut();
        let (base_r, base_g, base_b) = Self::rgb(theme.secondary);

        for (col, col_drops) in self.drops.iter().enumerate() {
            let amp = amplitudes[col].clamp(0.0, 1.0);
            // Minimum brightness multiplier even at low amplitude.
            let amp_bright = 0.4 + amp * 0.6;

            for drop in col_drops.iter() {
                // head is at drop.y, tail extends upward for drop.len rows.
                let head_row = drop.y.floor() as i32;

                for seg in 0..drop.len {
                    let row = head_row - seg as i32;
                    if row < 0 || row >= h as i32 {
                        continue;
                    }

                    let x = area.x + col as u16;
                    let y = area.y + row as u16;
                    if x >= area.x + area.width || y >= area.y + area.height {
                        continue;
                    }

                    // seg=0 is the head (brightest), seg=len-1 is the tail (dimmest).
                    let tail_fade = if drop.len <= 1 {
                        1.0f32
                    } else {
                        1.0 - (seg as f32 / (drop.len - 1) as f32)
                    };
                    // Head is full brightness; tail fades to ~10%.
                    let brightness = amp_bright * (0.1 + tail_fade * 0.9);

                    let ch = drop.chars.get(seg).copied().unwrap_or('|');
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
}
