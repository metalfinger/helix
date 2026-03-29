use crate::status::HelixState;
use crate::theme::Theme;
use crate::ambient::AmbientEffect;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget as RatatuiWidget;

const BRAILLE_CHARS: &[char] = &['⠁', '⠂', '⠄', '⡀', '⢀', '⠠', '⠐', '⠈'];
const SMALL_CHARS: &[char] = &['°', '•', '·'];
const SPARKLE_CHARS: &[char] = &['✦', '✧', '⋆', '∗'];

const MIN_FIREFLIES: usize = 20;
const MAX_FIREFLIES: usize = 30;
const TRAIL_LENGTH: usize = 2;
const TRAIL_BRIGHTNESS: f32 = 0.3;
const CLUSTER_RADIUS: f32 = 10.0;
const CLUSTER_FORCE: f32 = 0.002;
const FLASH_INTERVAL: u64 = 100;
const FLASH_DURATION: u64 = 20;

struct Firefly {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    brightness: f32,
    blink_phase: f32,
    color_offset_r: i8,
    color_offset_g: i8,
    color_offset_b: i8,
    trail: [(f32, f32); TRAIL_LENGTH],
    flash_timer: u64,
}

pub struct Fireflies {
    flies: Vec<Firefly>,
    width: u16,
    height: u16,
    idle_ratio: f32,
    seed: u64,
    fade_factor: f32,
    tick_count: u64,
}

impl Fireflies {
    pub fn new() -> Self {
        Self {
            flies: Vec::new(),
            width: 0,
            height: 0,
            idle_ratio: 0.0,
            seed: 12345,
            fade_factor: 1.0,
            tick_count: 0,
        }
    }

    pub fn set_idle_ratio(&mut self, ratio: f32) {
        self.idle_ratio = ratio.clamp(0.0, 1.0);
    }

    fn xorshift(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }

    fn random_f32(&mut self) -> f32 {
        (self.xorshift() % 10000) as f32 / 10000.0
    }

    fn spawn_firefly(&mut self) -> Firefly {
        let x = self.random_f32() * self.width as f32;
        let y = self.random_f32() * self.height as f32;
        let vx = (self.random_f32() - 0.5) * 0.4;
        let vy = (self.random_f32() - 0.5) * 0.3;
        let brightness = self.random_f32();
        let blink_phase = self.random_f32() * std::f32::consts::TAU;
        // Color variation: offset RGB by ±20 from base warm hue
        let color_offset_r = ((self.random_f32() - 0.5) * 40.0) as i8;
        let color_offset_g = ((self.random_f32() - 0.5) * 40.0) as i8;
        let color_offset_b = ((self.random_f32() - 0.5) * 40.0) as i8;
        Firefly {
            x, y, vx, vy, brightness, blink_phase,
            color_offset_r, color_offset_g, color_offset_b,
            trail: [(x, y); TRAIL_LENGTH],
            flash_timer: 0,
        }
    }

    pub fn set_size(&mut self, width: u16, height: u16) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.flies.clear();
        }
    }
}

impl AmbientEffect for Fireflies {
    fn tick(&mut self, _state: HelixState) {
        if self.width == 0 || self.height == 0 {
            return;
        }

        self.tick_count = self.tick_count.wrapping_add(1);

        // Only spawn when >= 50% idle
        let should_be_active = self.idle_ratio >= 0.5;

        // Fade factor: 1.0 when idle, approaches 0.0 when active
        if should_be_active {
            self.fade_factor = (self.fade_factor + 0.02).min(1.0);
        } else {
            self.fade_factor = (self.fade_factor - 0.03).max(0.0);
        }

        // Target count based on idle ratio
        let target_count = if should_be_active {
            let t = (self.idle_ratio - 0.5) * 2.0; // 0.0 at 50%, 1.0 at 100%
            MIN_FIREFLIES + ((MAX_FIREFLIES - MIN_FIREFLIES) as f32 * t) as usize
        } else {
            0
        };

        // Spawn/despawn
        while self.flies.len() < target_count {
            let fly = self.spawn_firefly();
            self.flies.push(fly);
        }
        while self.flies.len() > target_count && !self.flies.is_empty() {
            self.flies.pop();
        }

        // Occasional bright flash — pick one random firefly every ~FLASH_INTERVAL ticks
        if self.tick_count % FLASH_INTERVAL == 0 && !self.flies.is_empty() {
            let idx = (self.xorshift() as usize) % self.flies.len();
            self.flies[idx].flash_timer = FLASH_DURATION;
        }

        // Collect positions for clustering computation (avoid borrow issues)
        let positions: Vec<(f32, f32)> = self.flies.iter().map(|f| (f.x, f.y)).collect();

        // Update positions
        let w = self.width as f32;
        let h = self.height as f32;
        let speed_mult = self.fade_factor; // slow down when fading

        for (i, fly) in self.flies.iter_mut().enumerate() {
            // Save current position to trail (shift trail, newest at index 0)
            for t in (1..TRAIL_LENGTH).rev() {
                fly.trail[t] = fly.trail[t - 1];
            }
            fly.trail[0] = (fly.x, fly.y);

            // Random perturbation (inline xorshift to avoid borrow issues)
            let seed_val = (fly.x.to_bits() as u64)
                .wrapping_add(fly.y.to_bits() as u64)
                .wrapping_add(i as u64)
                .wrapping_add(1);
            let mut s = seed_val;
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let rx = ((s % 1000) as f32 / 1000.0 - 0.5) * 0.08;
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let ry = ((s % 1000) as f32 / 1000.0 - 0.5) * 0.06;

            fly.vx += rx;
            fly.vy += ry;

            // Gentle clustering — attract toward nearby fireflies
            for (j, &(ox, oy)) in positions.iter().enumerate() {
                if i == j {
                    continue;
                }
                let dx = ox - fly.x;
                let dy = oy - fly.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > 0.5 && dist < CLUSTER_RADIUS {
                    fly.vx += dx / dist * CLUSTER_FORCE;
                    fly.vy += dy / dist * CLUSTER_FORCE;
                }
            }

            fly.vx = fly.vx.clamp(-0.5, 0.5);
            fly.vy = fly.vy.clamp(-0.4, 0.4);

            fly.x += fly.vx * speed_mult;
            fly.y += fly.vy * speed_mult;

            // Wrap around
            if fly.x < 0.0 { fly.x += w; }
            if fly.x >= w { fly.x -= w; }
            if fly.y < 0.0 { fly.y += h; }
            if fly.y >= h { fly.y -= h; }

            // Blink
            fly.blink_phase += 0.08 + (fly.brightness * 0.04);
            if fly.blink_phase > std::f32::consts::TAU {
                fly.blink_phase -= std::f32::consts::TAU;
            }
            fly.brightness = (fly.blink_phase.sin() * 0.5 + 0.5).clamp(0.0, 1.0);

            // Flash: override brightness to full while flash_timer > 0
            if fly.flash_timer > 0 {
                // Fade from 1.0 back to normal over FLASH_DURATION ticks
                let flash_ratio = fly.flash_timer as f32 / FLASH_DURATION as f32;
                fly.brightness = fly.brightness.max(flash_ratio);
                fly.flash_timer -= 1;
            }
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme, _state: HelixState) {
        if self.flies.is_empty() || self.fade_factor < 0.01 {
            return;
        }

        let widget = FirefliesWidget {
            flies: &self.flies,
            accent: theme.accent,
            fade_factor: self.fade_factor,
        };
        frame.render_widget(widget, area);
    }
}

struct FirefliesWidget<'a> {
    flies: &'a [Firefly],
    accent: Color,
    fade_factor: f32,
}

impl<'a> RatatuiWidget for FirefliesWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Warm tint on the accent color
        let (ar, ag, ab) = match self.accent {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (255, 180, 80),
        };
        // Shift toward warm (increase red, keep green, reduce blue)
        let warm_r = ((ar as u16 + 40).min(255)) as u8;
        let warm_g = ((ag as u16 + 10).min(255)) as u8;
        let warm_b = (ab as i16 - 20).max(0) as u8;

        for fly in self.flies {
            // Render trail positions at reduced brightness
            for trail_pos in &fly.trail {
                let tx = trail_pos.0 as u16;
                let ty = trail_pos.1 as u16;
                if tx >= area.width || ty >= area.height {
                    continue;
                }
                let screen_tx = area.x + tx;
                let screen_ty = area.y + ty;
                let tb = fly.brightness * self.fade_factor * TRAIL_BRIGHTNESS;
                if tb < 0.05 {
                    continue;
                }
                // Apply per-firefly color variation to trail
                let tr = ((warm_r as i16 + fly.color_offset_r as i16).clamp(0, 255) as f32 * tb) as u8;
                let tg = ((warm_g as i16 + fly.color_offset_g as i16).clamp(0, 255) as f32 * tb) as u8;
                let tbl = ((warm_b as i16 + fly.color_offset_b as i16).clamp(0, 255) as f32 * tb) as u8;
                let trail_color = Color::Rgb(tr, tg, tbl);
                // Use a dim dot for the trail
                let trail_ch = '·';
                if screen_tx < area.x + area.width && screen_ty < area.y + area.height {
                    buf.cell_mut((screen_tx, screen_ty))
                        .map(|cell| {
                            cell.set_char(trail_ch);
                            cell.set_style(Style::default().fg(trail_color));
                        });
                }
            }

            // Render the firefly itself
            let sx = fly.x as u16;
            let sy = fly.y as u16;

            if sx >= area.width || sy >= area.height {
                continue;
            }

            let screen_x = area.x + sx;
            let screen_y = area.y + sy;

            let b = fly.brightness * self.fade_factor;
            if b < 0.1 {
                continue;
            }

            // Apply per-firefly color variation
            let fly_r = (warm_r as i16 + fly.color_offset_r as i16).clamp(0, 255) as u8;
            let fly_g = (warm_g as i16 + fly.color_offset_g as i16).clamp(0, 255) as u8;
            let fly_b = (warm_b as i16 + fly.color_offset_b as i16).clamp(0, 255) as u8;

            let scale = b;
            let r = (fly_r as f32 * scale) as u8;
            let g = (fly_g as f32 * scale) as u8;
            let bl = (fly_b as f32 * scale) as u8;
            let color = Color::Rgb(r, g, bl);

            // Pick character based on brightness:
            // Bright (>0.8): big sparkle chars
            // Medium (0.4-0.8): small dot chars
            // Dim (<0.4): braille dots
            let ch = if b > 0.8 {
                SPARKLE_CHARS[((fly.x as usize) + (fly.y as usize)) % SPARKLE_CHARS.len()]
            } else if b > 0.4 {
                SMALL_CHARS[((fly.x as usize) + (fly.y as usize)) % SMALL_CHARS.len()]
            } else {
                BRAILLE_CHARS[((fly.x as usize) + (fly.y as usize)) % BRAILLE_CHARS.len()]
            };

            if screen_x < area.x + area.width && screen_y < area.y + area.height {
                buf.cell_mut((screen_x, screen_y))
                    .map(|cell| {
                        cell.set_char(ch);
                        cell.set_style(Style::default().fg(color));
                    });
            }
        }
    }
}
