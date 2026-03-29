use crate::status::HelixState;
use crate::theme::Theme;
use crate::ambient::AmbientEffect;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget as RatatuiWidget;

const KATAKANA: &[char] = &[
    'ア', 'イ', 'ウ', 'エ', 'オ', 'カ', 'キ', 'ク', 'ケ', 'コ',
    'サ', 'シ', 'ス', 'セ', 'ソ', 'タ', 'チ', 'ツ', 'テ', 'ト',
    'ナ', 'ニ', 'ヌ', 'ネ', 'ノ', 'ハ', 'ヒ', 'フ', 'ヘ', 'ホ',
    'マ', 'ミ', 'ム', 'メ', 'モ', 'ヤ', 'ユ', 'ヨ', 'ラ', 'リ',
    'ル', 'レ', 'ロ', 'ワ', 'ヲ', 'ン',
];

const LATIN_DIGITS: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    'A', 'B', 'C', 'D', 'E', 'F', 'a', 'b', 'c', 'd',
];

struct Drop {
    x: u16,
    y: f32,
    speed: f32,
    length: u16,
    chars: Vec<char>,
}

pub struct MatrixRain {
    drops: Vec<Drop>,
    width: u16,
    height: u16,
    density: f32,
    target_density: f32,
    seed: u64,
    /// Override rain color with active session color
    pub color_override: Option<Color>,
    /// Audio energy level (0.0–1.0) for sound-reactive effects
    pub audio_energy: f32,
}

impl MatrixRain {
    pub fn new() -> Self {
        Self {
            drops: Vec::new(),
            width: 0,
            height: 0,
            density: 0.1,
            target_density: 0.1,
            seed: 42,
            color_override: None,
            audio_energy: 0.0,
        }
    }

    /// Set audio energy level for sound-reactive effects
    pub fn set_audio_energy(&mut self, energy: f32) {
        self.audio_energy = energy;
    }

    /// Set rain density based on activity level (0.0 = dead, 1.0 = max)
    pub fn set_activity(&mut self, active_count: usize, total_count: usize) {
        self.target_density = if total_count == 0 {
            0.08 // light rain when no sessions
        } else {
            let ratio = active_count as f32 / total_count as f32;
            0.08 + ratio * 0.12 // 0.08 (idle) → 0.20 (all active)
        };
    }

    pub fn set_size(&mut self, width: u16, height: u16) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.drops.clear();
            self.ensure_drops(width, height);
        }
    }

    fn pseudo_random(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }

    fn random_char(&mut self) -> char {
        let r = self.pseudo_random() as usize;
        if r % 3 == 0 {
            LATIN_DIGITS[r % LATIN_DIGITS.len()]
        } else {
            KATAKANA[r % KATAKANA.len()]
        }
    }

    fn ensure_drops(&mut self, width: u16, height: u16) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.drops.clear();

            let num_drops = (width as f32 * self.density) as usize;
            for i in 0..num_drops {
                self.seed = (i as u64).wrapping_mul(7919).wrapping_add(13);
                let x = (self.pseudo_random() % width as u64) as u16;
                let speed = 0.2 + (self.pseudo_random() % 100) as f32 / 200.0;
                let length = 4 + (self.pseudo_random() % (height as u64 / 2).max(1)) as u16;
                let y = -((self.pseudo_random() % (height as u64 * 2)) as f32);
                let chars: Vec<char> = (0..length).map(|_| self.random_char()).collect();

                self.drops.push(Drop { x, y, speed, length, chars });
            }
        }
    }

    fn state_color(&self, state: HelixState, theme: &Theme) -> Color {
        match state {
            HelixState::Idle => theme.primary,
            HelixState::Thinking => theme.thinking,
            HelixState::Coding => theme.success,
            HelixState::Reviewing => theme.secondary,
            HelixState::Committing => Color::Rgb(255, 215, 0), // gold
            HelixState::Streaming => theme.secondary,
            HelixState::Error => theme.error,
            HelixState::Done => theme.success,
            HelixState::Deep => Color::Rgb(60, 80, 170),
            HelixState::Critical => theme.error,
        }
    }

    fn state_speed_mult(&self, state: HelixState) -> f32 {
        match state {
            HelixState::Idle => 0.7,
            HelixState::Thinking => 1.5,
            HelixState::Coding => 1.2,
            HelixState::Error => 0.3,
            HelixState::Critical => 0.2,
            HelixState::Done => 2.0,
            _ => 1.0,
        }
    }
}

impl MatrixRain {
    /// Render rain as overlay on top of existing content
    pub fn render_overlay(&self, frame: &mut Frame, area: Rect, theme: &Theme, state: HelixState) {
        let base = self.color_override.unwrap_or_else(|| self.state_color(state, theme));
        let widget = MatrixRainOverlayWidget {
            drops: &self.drops,
            base_color: base,
        };
        frame.render_widget(widget, area);
    }
}

impl AmbientEffect for MatrixRain {
    fn tick(&mut self, state: HelixState) {
        // Initialize drops if empty (first tick or resize)
        if self.drops.is_empty() && self.width > 0 {
            self.ensure_drops(self.width, self.height);
        }

        // Apply audio-reactive modifiers to density target
        let effective_target = if self.audio_energy > 0.01 {
            self.target_density * (1.0 + self.audio_energy * 2.0)
        } else {
            self.target_density
        };

        // Smoothly lerp density toward target
        let diff = effective_target - self.density;
        if diff.abs() > 0.005 {
            self.density += diff * 0.05; // slow smooth transition
            // Adjust drop count
            let target_drops = (self.width as f32 * self.density) as usize;
            while self.drops.len() < target_drops && self.width > 0 {
                self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(7);
                let x = (self.pseudo_random() % self.width as u64) as u16;
                let speed = 0.2 + (self.pseudo_random() % 100) as f32 / 200.0;
                let length = 4 + (self.pseudo_random() % (self.height as u64 / 2).max(1)) as u16;
                let chars: Vec<char> = (0..length).map(|_| self.random_char()).collect();
                self.drops.push(Drop { x, y: -(length as f32), speed, length, chars });
            }
            while self.drops.len() > target_drops && !self.drops.is_empty() {
                self.drops.pop();
            }
        }

        let speed_mult = self.state_speed_mult(state);
        // Audio-reactive speed boost
        let speed_mult = if self.audio_energy > 0.01 {
            speed_mult * (0.8 + self.audio_energy * 1.5)
        } else {
            speed_mult
        };
        let height = self.height as f32;

        for drop in &mut self.drops {
            drop.y += drop.speed * speed_mult;

            if drop.y > height + drop.length as f32 {
                drop.y = -(drop.length as f32);
                // Shuffle a char
                let idx = (drop.y.abs() as usize) % drop.chars.len().max(1);
                if idx < drop.chars.len() {
                    self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let r = self.seed as usize;
                    drop.chars[idx] = if r % 3 == 0 {
                        LATIN_DIGITS[r % LATIN_DIGITS.len()]
                    } else {
                        KATAKANA[r % KATAKANA.len()]
                    };
                }
            }
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme, state: HelixState) {
        // Lazy init drops on first render when we know the size
        // Safety: we need &mut self but trait gives &self, so we use the render area
        // to determine if drops exist. If empty, the tick() will populate next frame.
        let base = self.color_override.unwrap_or_else(|| self.state_color(state, theme));
        let rain_widget = MatrixRainWidget {
            drops: &self.drops,
            base_color: base,
        };
        frame.render_widget(rain_widget, area);
    }
}

struct MatrixRainWidget<'a> {
    drops: &'a [Drop],
    base_color: Color,
}

impl<'a> RatatuiWidget for MatrixRainWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_drops(self.drops, self.base_color, area, buf, false);
    }
}

/// Overlay widget: renders rain on top of existing content without clearing
struct MatrixRainOverlayWidget<'a> {
    drops: &'a [Drop],
    base_color: Color,
}

impl<'a> RatatuiWidget for MatrixRainOverlayWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_drops(self.drops, self.base_color, area, buf, true);
    }
}

/// Shared render logic for both background and overlay modes
fn render_drops(drops: &[Drop], base_color: Color, area: Rect, buf: &mut Buffer, overlay: bool) {
    for drop in drops {
        let head_y = drop.y as i32;

        for (i, ch) in drop.chars.iter().enumerate() {
            let y = head_y - i as i32;
            if y < 0 || y >= area.height as i32 {
                continue;
            }
            // Katakana chars are 2 cells wide, skip if they'd overflow
            let char_width = if *ch as u32 > 0x30A0 { 2 } else { 1 };
            if drop.x + char_width as u16 > area.width {
                continue;
            }

            let screen_y = area.y + y as u16;
            let screen_x = area.x + drop.x;

            // Brightness fades along the tail
            let brightness = if i == 0 {
                255 // head is brightest (white flash)
            } else {
                let fade = 1.0 - (i as f32 / drop.length as f32);
                (fade * 180.0) as u8
            };

            // In overlay mode, dim the rain more so content shows through
            let brightness = if overlay { brightness / 2 } else { brightness };

            let color = if i == 0 {
                if overlay { Color::Rgb(180, 255, 180) } else { Color::Rgb(255, 255, 255) }
            } else {
                match base_color {
                    Color::Rgb(r, g, b) => {
                        let scale = brightness as f32 / 255.0;
                        Color::Rgb(
                            (r as f32 * scale) as u8,
                            (g as f32 * scale) as u8,
                            (b as f32 * scale) as u8,
                        )
                    }
                    _ => base_color,
                }
            };

            if screen_x < area.x + area.width && screen_y < area.y + area.height {
                buf.cell_mut((screen_x, screen_y))
                    .map(|cell| {
                        cell.set_char(*ch);
                        cell.set_style(Style::default().fg(color));
                    });
            }
        }
    }
}
