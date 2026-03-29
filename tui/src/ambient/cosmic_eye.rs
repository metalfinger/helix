use crate::status::HelixState;
use crate::theme::Theme;
use crate::ambient::AmbientEffect;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget as RatatuiWidget;

/// Cosmic Eye — port of Shadertoy wX3Gz8
/// Animated eye with aurora/stars in pupil, blinking lids, eyelashes, wandering gaze
pub struct CosmicEye {
    time: f32,
    look_x: f32,
    look_y: f32,
    target_x: f32,
    target_y: f32,
    saccade_timer: f32,
    /// 1.0 = fully open, 0.0 = fully closed
    lid_openness: f32,
    /// Countdown: when >0, eye is blinking closed then reopening
    blink_timer: f32,
    /// Time until next blink
    next_blink: f32,
    rng: u32,
    tick: u64,
}

impl CosmicEye {
    pub fn new() -> Self {
        Self {
            time: 0.0,
            look_x: 0.0,
            look_y: 0.0,
            target_x: 0.0,
            target_y: 0.0,
            saccade_timer: 2.0,
            lid_openness: 1.0,
            blink_timer: 0.0,
            next_blink: 3.0,
            rng: 12345,
            tick: 0,
        }
    }

    fn frand(&mut self) -> f32 {
        let state = self.rng.wrapping_mul(747796405).wrapping_add(2891336453);
        let word = ((state >> ((state >> 28).wrapping_add(4))) ^ state).wrapping_mul(277803737);
        self.rng = (word >> 22) ^ word;
        self.rng as f32 / u32::MAX as f32
    }
}

impl AmbientEffect for CosmicEye {
    fn tick(&mut self, _state: HelixState) {
        let dt = 0.066_f32;
        self.time += dt;
        self.tick += 1;

        // Blink: countdown to next blink, then quick close-open
        self.next_blink -= dt;
        if self.next_blink <= 0.0 {
            self.blink_timer = 0.3; // 0.3s blink duration
            self.next_blink = 2.5 + self.frand() * 4.0; // 2.5-6.5s between blinks
        }

        if self.blink_timer > 0.0 {
            self.blink_timer -= dt;
            // Triangle wave: close then open over 0.3s
            let half = 0.15;
            let elapsed = 0.3 - self.blink_timer;
            self.lid_openness = if elapsed < half {
                // Closing: 1.0 → 0.05
                1.0 - (elapsed / half) * 0.95
            } else {
                // Opening: 0.05 → 1.0
                0.05 + ((elapsed - half) / half) * 0.95
            };
        } else {
            self.lid_openness = 1.0;
        }

        // Saccade — eye wanders
        self.saccade_timer -= dt;
        if self.saccade_timer <= 0.0 {
            self.target_x = self.frand() * 0.5 - 0.25;
            self.target_y = self.frand() * 0.3 - 0.15;
            self.saccade_timer = 1.5 + self.frand() * 3.0;
        }
        self.look_x += (self.target_x - self.look_x) * 0.1;
        self.look_y += (self.target_y - self.look_y) * 0.1;
    }

    fn render(&self, frame: &mut Frame, area: Rect, _theme: &Theme, _state: HelixState) {
        let widget = CosmicEyeWidget {
            time: self.time,
            look_x: self.look_x,
            look_y: self.look_y,
            lid_openness: self.lid_openness,
            tick: self.tick,
        };
        frame.render_widget(widget, area);
    }
}

struct CosmicEyeWidget {
    time: f32,
    look_x: f32,
    look_y: f32,
    lid_openness: f32,
    tick: u64,
}

// --- Math helpers ---

fn rand_2d(nx: f32, ny: f32) -> f32 {
    let dot = nx * 12.9898 + ny * 4.1414;
    (dot.sin() * 43758.5453).fract().abs()
}

fn noise_2d(px: f32, py: f32) -> f32 {
    let ix = px.floor();
    let iy = py.floor();
    let mut ux = px - ix;
    let mut uy = py - iy;
    if ux < 0.0 { ux += 1.0; }
    if uy < 0.0 { uy += 1.0; }
    let ux = ux * ux * (3.0 - 2.0 * ux);
    let uy = uy * uy * (3.0 - 2.0 * uy);

    let a = rand_2d(ix, iy);
    let b = rand_2d(ix + 1.0, iy);
    let c = rand_2d(ix, iy + 1.0);
    let d = rand_2d(ix + 1.0, iy + 1.0);

    let res = mix(mix(a, b, ux), mix(c, d, ux), uy);
    res * res
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Eyelid shape: gaussian curve. Returns the lid height at position x.
/// openness: 0=closed, 1=fully open
fn lid_height(x: f32, openness: f32) -> f32 {
    let s = 1.5_f32;
    let base = (-(x / s).powi(2)).exp();
    // Scale by openness — when closed, lid height → 0
    (base * openness).max(0.0)
}

/// FBM for iris/nebula coloring (2 octaves for performance)
fn fbm_simple(px: f32, py: f32, time: f32) -> f32 {
    let mut f = 0.0_f32;
    f += 0.5 * noise_2d(px + time * 0.4, py + time * 0.2);
    let (rx, ry) = (0.80 * px + 0.60 * py, -0.60 * px + 0.80 * py);
    f += 0.25 * noise_2d(rx * 2.0, ry * 2.0);
    f / 0.75
}

/// Fire-like colormap from the original shader
fn colormap(x: f32) -> (f32, f32, f32) {
    let r = if x < 0.241 { (829.79 * x + 54.51) / 255.0 } else { 1.0 };
    let g = if x < 0.241 {
        0.0
    } else if x < 0.403 {
        ((x - 0.241) / (0.403 - 0.241)).min(1.0) * 0.6
    } else {
        (0.6 + (x - 0.403) * 0.67).min(1.0)
    };
    let b = if x < 0.087 {
        (829.79 * x + 54.51) / 255.0
    } else if x < 0.241 {
        0.5
    } else if x < 0.403 {
        ((x - 0.1) * 2.0).min(1.0)
    } else {
        1.0
    };
    (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0))
}

impl RatatuiWidget for CosmicEyeWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 8 || area.height < 4 {
            return;
        }

        let w = area.width as f32;
        let h = area.height as f32;
        let time = self.time;
        let openness = self.lid_openness;

        // Coordinate system: eye spans ~±1.5 horizontally, ±1.0 vertically
        // Use the smaller dimension to keep the eye proportional
        let scale_x = 3.5 / w;           // maps width to -1.75..1.75
        let scale_y = 3.5 / h * 2.0;     // aspect correction for terminal cells

        for sy in area.y..area.y + area.height {
            for sx in area.x..area.x + area.width {
                let px = sx as f32 - area.x as f32;
                let py = sy as f32 - area.y as f32;

                let ux = (px - 0.5 * w) * scale_x;
                let uy = (py - 0.5 * h) * scale_y;

                // Eyelid boundaries
                let l = lid_height(ux, openness);

                let mut r = 0.0_f32;
                let mut g = 0.0_f32;
                let mut b = 0.0_f32;
                let mut drawn = false;

                // Lid edge (white outline of the eye shape) — always visible
                if ux.abs() < 2.0 {
                    let dist_top = (uy - l).abs();
                    let dist_bot = (uy + l).abs();
                    let edge = dist_top.min(dist_bot);
                    let edge_thickness = 0.12;

                    if edge < edge_thickness {
                        let intensity = 1.0 - (edge / edge_thickness);
                        r = intensity;
                        g = intensity;
                        b = intensity;
                        drawn = true;
                    }
                }

                // Eyelashes (above the upper lid)
                if !drawn && ux.abs() < 2.0 {
                    let lash_count = 7_i32;
                    for i in 0..lash_count {
                        let lash_x = -1.6 + 3.2 * (i as f32 / (lash_count - 1) as f32);
                        let base_y = lid_height(lash_x, openness);

                        // Lash direction: points outward/upward
                        let dx = ux - lash_x;
                        let dy = uy - base_y;

                        // Lashes go upward (negative uy direction means up visually in shader coords)
                        // but our uy has negative = top, so lashes go to more negative uy
                        if dy > 0.0 && dy < 0.4 && dx.abs() < 0.08 * (1.0 - dy / 0.4) {
                            r = 0.9;
                            g = 0.9;
                            b = 0.9;
                            drawn = true;
                            break;
                        }
                    }
                }

                // Inside the eye opening
                if !drawn && uy > -l && uy < l && ux.abs() < 1.8 {
                    drawn = true;

                    // Eyeball coordinates relative to iris center (shifted by gaze)
                    let ex = ux - self.look_x;
                    let ey = uy - self.look_y;
                    let dist = (ex * ex + ey * ey).sqrt();

                    let eyeball_r = 1.2;
                    let iris_r = 0.75;
                    let pupil_r = 0.35;

                    if dist > eyeball_r {
                        // Sclera edge — slight pink tint
                        r = 0.7;
                        g = 0.65;
                        b = 0.65;
                    } else if dist > iris_r {
                        // Sclera (white of eye)
                        let sclera_shade = 1.0 - (dist / eyeball_r) * 0.3;
                        r = sclera_shade;
                        g = sclera_shade;
                        b = sclera_shade;
                    } else if dist > pupil_r + 0.05 {
                        // Iris — animated interference rings + color
                        let ring_d = dist - pupil_r;
                        let interference = (ring_d * 40.0 + time * 15.0).sin()
                            + (ring_d * 25.0 - time * 8.0).sin()
                            + (ring_d * 80.0 + time * 5.0).sin();

                        // Iris base color (amber/brown with noise variation)
                        let iris_noise = noise_2d(ex * 6.0 + time * 0.2, ey * 6.0);
                        let angle = ey.atan2(ex);
                        let radial = ((angle * 12.0 + time * 2.0).sin() * 0.3 + 0.7).max(0.0);

                        r = 0.6 * radial + iris_noise * 0.3;
                        g = 0.35 * radial + iris_noise * 0.15;
                        b = 0.1 + iris_noise * 0.1;

                        // Bright streaks from interference
                        if interference / 3.0 > 0.3 {
                            let boost = (interference / 3.0 - 0.3) * 1.5;
                            r = (r + boost * 0.8).min(1.0);
                            g = (g + boost * 0.6).min(1.0);
                            b = (b + boost * 0.3).min(1.0);
                        }
                    } else if dist > pupil_r {
                        // Pupil edge — dark ring
                        r = 0.02;
                        g = 0.02;
                        b = 0.02;
                    } else {
                        // Pupil interior — stars + aurora + nebula

                        // Deep space background
                        let bg_angle = (-0.5 * ex - 0.6 * ey).max(0.0).min(1.0);
                        r = mix(0.03, 0.08, bg_angle.powi(3));
                        g = mix(0.05, 0.03, bg_angle.powi(3));
                        b = mix(0.12, 0.15, bg_angle.powi(3));

                        // Star field
                        let star_uv_x = ex * 8.0;
                        let star_uv_y = ey * 8.0;
                        let star_cell_x = star_uv_x.floor();
                        let star_cell_y = star_uv_y.floor();
                        let star_frac_x = star_uv_x - star_cell_x;
                        let star_frac_y = star_uv_y - star_cell_y;
                        let star_hash = rand_2d(star_cell_x + time.floor() * 0.01, star_cell_y);
                        if star_hash < 0.08 {
                            let cdist = ((star_frac_x - 0.5).powi(2) + (star_frac_y - 0.5).powi(2)).sqrt();
                            let star_bright = (1.0 - cdist * 3.0).max(0.0);
                            let twinkle = ((time * 3.0 + star_cell_x * 7.0 + star_cell_y * 13.0).sin() * 0.3 + 0.7).max(0.0);
                            r += star_bright * twinkle;
                            g += star_bright * twinkle;
                            b += star_bright * twinkle;
                        }

                        // Aurora / nebula bands
                        let aurora_y = ey / pupil_r; // -1..1
                        let aurora_wave = (aurora_y * 5.0 + time * 0.8).sin() * 0.5 + 0.5;
                        let aurora_noise = noise_2d(ex * 4.0 + time * 0.5, ey * 3.0 + time * 0.3);
                        let aurora_strength = aurora_wave * aurora_noise * 0.7;

                        r += aurora_strength * 0.3;
                        g += aurora_strength * 0.6;
                        b += aurora_strength * 0.9;

                        // Colormap overlay (fire pattern from original shader)
                        let pat = fbm_simple(ux * 0.8, uy * 0.8, time);
                        let (cr, cg, cb) = colormap(pat);
                        r = r.powf(0.8) + cr * 0.25;
                        g = g.powf(0.8) + cg * 0.25;
                        b = b.powf(0.8) + cb * 0.25;
                    }
                }

                if !drawn {
                    continue;
                }

                // Clamp
                r = r.clamp(0.0, 1.0);
                g = g.clamp(0.0, 1.0);
                b = b.clamp(0.0, 1.0);

                let brightness = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                if brightness < 0.008 {
                    continue;
                }

                let ch = if brightness > 0.8 {
                    '█'
                } else if brightness > 0.6 {
                    '▓'
                } else if brightness > 0.4 {
                    '▒'
                } else if brightness > 0.2 {
                    '░'
                } else if brightness > 0.08 {
                    '·'
                } else {
                    ' '
                };

                let cr = (r * 255.0) as u8;
                let cg = (g * 255.0) as u8;
                let cb = (b * 255.0) as u8;

                buf.cell_mut((sx, sy)).map(|cell| {
                    cell.set_char(ch);
                    cell.set_style(Style::default().fg(Color::Rgb(cr, cg, cb)));
                });
            }
        }
    }
}
