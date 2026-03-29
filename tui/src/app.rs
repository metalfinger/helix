use crate::config::Config;
use crate::history;
use crate::memory_state::MemoryWatcher;
use crate::widgets::finance_overlay::FinanceState;
use crate::widgets::memory_explorer::{ExplorerState, ExplorerCmdResult, ToastLevel};
use crate::status::{self, SessionStatus, StatusWatcher};
use crate::theme::Theme;
use crate::widgets::activity_feed::{FeedEntry, EntryStatus};
use crate::widgets::visualizer::audio_capture::AudioCapture;
use crate::widgets::visualizer::bar_spectrum::BarSpectrum;
use crate::widgets::visualizer::circular::Circular;
use crate::widgets::visualizer::dot_matrix::DotMatrix;
use crate::widgets::visualizer::fire::Fire;
use crate::widgets::visualizer::heartbeat::Heartbeat;
use crate::widgets::visualizer::kaleidoscope::Kaleidoscope;
use crate::widgets::visualizer::particle_field::ParticleField;
use crate::widgets::visualizer::rainfall::Rainfall;
use crate::widgets::visualizer::scope::Scope;
use crate::widgets::visualizer::spectrogram::Spectrogram;
use crate::widgets::visualizer::stereo::Stereo;
use crate::widgets::visualizer::vu_meter::VuMeter;
use crate::widgets::visualizer::VisualizerStyle;
use crate::ambient::AmbientEffect;
use crate::ambient::breathing_glow::BreathingGlow;
use crate::ambient::cosmic_eye::CosmicEye;
use crate::ambient::fireflies::Fireflies;
use crate::ambient::fractal_plasma::FractalPlasma;
use crate::ambient::lava_lamp::LavaLamp;
use crate::ambient::matrix_rain::MatrixRain;
use crate::scanner::{ProcessScanner, DetectedSession};
use crate::sessions;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, MouseEventKind, EnableMouseCapture, DisableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayMode {
    None,
    Activity,
    History,
    MemoryExplorer,
    Finance,
}

pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub statuses: Arc<Mutex<Vec<SessionStatus>>>,
    pub feed_entries: Arc<Mutex<Vec<FeedEntry>>>,
    pub matrix_rain: MatrixRain,
    pub detected_sessions: Vec<DetectedSession>,
    pub session_data: Option<sessions::SessionData>,
    pub scanner: ProcessScanner,
    pub last_scan: Instant,
    pub running: bool,
    pub tick_count: u64,
    /// Track last seen activity per session (by cwd) to detect new entries
    last_activity_keys: std::collections::HashMap<String, String>,
    /// Per-session token usage history for sparkline (cwd → last 20 used_pct values)
    pub token_history: std::collections::HashMap<String, Vec<u32>>,
    pub overlay_mode: OverlayMode,
    pub overlay_scroll: u16,
    pub history_entries: Vec<history::HistoryEntry>,
    pub history_selected: usize,
    pub history_delete_confirm: bool,
    pub files_touched: std::collections::HashMap<String, std::collections::HashSet<String>>,
    pub audio_capture: AudioCapture,
    pub visualizer_styles: Arc<Mutex<Vec<Box<dyn VisualizerStyle>>>>,
    pub active_visualizer: usize,
    pub rain_visible: bool,
    pub visualizer_visible: bool,
    pub breathing_glow: BreathingGlow,
    pub glow_visible: bool,
    pub fireflies: Fireflies,
    pub fireflies_visible: bool,
    pub lava_lamp: LavaLamp,
    pub lava_visible: bool,
    pub fractal_plasma: FractalPlasma,
    pub plasma_visible: bool,
    pub cosmic_eye: CosmicEye,
    pub eye_visible: bool,
    pub memory_watcher: MemoryWatcher,
    pub memory_visible: bool,
    pub explorer_state: ExplorerState,
    pub explorer_cmd_tx: tokio::sync::mpsc::UnboundedSender<ExplorerCmdResult>,
    pub explorer_cmd_rx: Option<tokio::sync::mpsc::UnboundedReceiver<ExplorerCmdResult>>,
    pub finance_state: FinanceState,
    pub wallet_pending: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        let theme = Theme::by_name(&config.general.theme);
        let mut app = Self {
            config,
            theme,
            statuses: Arc::new(Mutex::new(Vec::new())),
            feed_entries: Arc::new(Mutex::new(Vec::new())),
            matrix_rain: MatrixRain::new(),
            detected_sessions: Vec::new(),
            session_data: None,
            scanner: ProcessScanner::new(),
            last_scan: Instant::now(),
            running: true,
            tick_count: 0,
            last_activity_keys: std::collections::HashMap::new(),
            token_history: std::collections::HashMap::new(),
            overlay_mode: OverlayMode::None,
            overlay_scroll: 0,
            history_entries: Vec::new(),
            history_selected: 0,
            history_delete_confirm: false,
            files_touched: std::collections::HashMap::new(),
            audio_capture: AudioCapture::new(),
            visualizer_styles: Arc::new(Mutex::new(vec![
                Box::new(BarSpectrum::new()),
                Box::new(ParticleField::new()),
                Box::new(Scope::new()),
                Box::new(VuMeter::new()),
                Box::new(Spectrogram::new()),
                Box::new(Circular::new()),
                Box::new(Stereo::new()),
                Box::new(Fire::new()),
                Box::new(DotMatrix::new()),
                Box::new(Heartbeat::new()),
                Box::new(Rainfall::new()),
                Box::new(Kaleidoscope::new()),
            ])),
            active_visualizer: 0,
            rain_visible: false,
            visualizer_visible: false,
            breathing_glow: BreathingGlow::new(),
            glow_visible: false,
            fireflies: Fireflies::new(),
            fireflies_visible: false,
            lava_lamp: LavaLamp::new(),
            lava_visible: false,
            fractal_plasma: FractalPlasma::new(),
            plasma_visible: false,
            cosmic_eye: CosmicEye::new(),
            eye_visible: false,
            memory_watcher: MemoryWatcher::new(),
            memory_visible: false,
            explorer_state: ExplorerState::new(),
            explorer_cmd_tx: {
                let (tx, _) = tokio::sync::mpsc::unbounded_channel();
                tx
            },
            explorer_cmd_rx: None,
            finance_state: FinanceState::new(),
            wallet_pending: false,
        };
        // Re-create channel properly (can't destructure in struct init)
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        app.explorer_cmd_tx = tx;
        app.explorer_cmd_rx = Some(rx);

        // Startup pruning: remove history entries older than 30 days
        let mut startup_history = history::read_history();
        history::prune_history(&mut startup_history);

        app
    }

    pub async fn run(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic| {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
            original_hook(panic);
        }));

        // Watch the ~/.ai-status/ directory
        let status_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".ai-status");
        let _ = std::fs::create_dir_all(&status_dir);
        let watcher = StatusWatcher::new(status_dir.clone());
        self.statuses = watcher.statuses.clone();

        let (tx, mut rx) = mpsc::channel(32);
        watcher.watch(tx, self.config.general.debounce_ms).await;

        // Watch ~/.helix/state/ for memory data
        self.memory_watcher.load_initial();
        let (mem_tx, mut mem_rx) = mpsc::channel(8);
        self.memory_watcher.watch(mem_tx, 500).await;

        // Initial scan
        self.detected_sessions = self.scanner.scan();
        self.refresh_session_data();

        let tick_rate = Duration::from_millis(1000 / self.config.general.fps as u64);

        while self.running {
            let tick_start = Instant::now();

            let statuses = self.statuses.lock().unwrap().clone();

            let primary_status = self.primary_status(&statuses);
            let helix_state = primary_status.helix_state();

            let size = terminal.size()?;
            self.matrix_rain.set_size(size.width, size.height);
            self.fireflies.set_size(size.width, size.height);

            // Set rain density based on how many sessions are actively working
            let now_ts = chrono::Utc::now().timestamp() as u64;
            let active_count = statuses.iter()
                .filter(|s| s.cli == "claude-code" && !s.state.is_empty() && s.state != "idle" && now_ts.saturating_sub(s.timestamp) <= 5)
                .count();
            let total_count = statuses.iter().filter(|s| s.cli == "claude-code" && !s.cwd.is_empty()).count();
            self.matrix_rain.set_activity(active_count, total_count);

            // Set rain color to the most active session's color
            if let Some(active) = statuses.iter()
                .filter(|s| s.cli == "claude-code" && !s.cwd.is_empty() && now_ts.saturating_sub(s.timestamp) <= 5)
                .max_by_key(|s| s.timestamp)
            {
                let hash = status::fnv1a_32(status::normalize_cwd(&active.cwd).as_bytes());
                const RAIN_COLORS: &[ratatui::style::Color] = &[
                    ratatui::style::Color::Rgb(60, 120, 120),   // Cyan (dimmed)
                    ratatui::style::Color::Rgb(122, 101, 32),   // Amber
                    ratatui::style::Color::Rgb(110, 48, 80),    // Rose
                    ratatui::style::Color::Rgb(58, 110, 48),    // Lime
                    ratatui::style::Color::Rgb(80, 48, 120),    // Violet
                    ratatui::style::Color::Rgb(48, 88, 120),    // Ice
                    ratatui::style::Color::Rgb(120, 48, 32),    // Ember
                    ratatui::style::Color::Rgb(74, 96, 64),     // Moss
                ];
                self.matrix_rain.color_override = Some(RAIN_COLORS[(hash as usize) % RAIN_COLORS.len()]);
            } else {
                self.matrix_rain.color_override = None;
            }

            // Feed audio energy into matrix rain for sound-reactive effects
            if self.rain_visible {
                let audio_bands = self.audio_capture.bands.lock().unwrap();
                let energy = audio_bands.iter().sum::<f32>() / audio_bands.len().max(1) as f32;
                drop(audio_bands);
                self.matrix_rain.set_audio_energy(energy);
            } else {
                self.matrix_rain.set_audio_energy(0.0);
            }

            self.matrix_rain.tick(helix_state);
            self.breathing_glow.tick(helix_state);

            // Fireflies: calculate idle ratio and tick
            let idle_ratio = if total_count == 0 {
                1.0
            } else {
                1.0 - (active_count as f32 / total_count as f32)
            };
            self.fireflies.set_idle_ratio(idle_ratio);
            self.fireflies.tick(helix_state);

            // Fractal plasma tick
            self.fractal_plasma.tick(helix_state);

            // Cosmic eye tick
            self.cosmic_eye.tick(helix_state);

            // Lava lamp tick
            self.lava_lamp.tick(helix_state);
            self.lava_lamp.set_size(size.width, size.height);

            // Collect session colors for lava lamp
            {
                let session_colors: Vec<ratatui::style::Color> = statuses.iter()
                    .filter(|s| s.cli == "claude-code" && !s.cwd.is_empty())
                    .map(|s| {
                        let hash = crate::status::fnv1a_32(crate::status::normalize_cwd(&s.cwd).as_bytes());
                        const PALETTES: &[ratatui::style::Color] = &[
                            ratatui::style::Color::Rgb(120, 255, 255),
                            ratatui::style::Color::Rgb(255, 208, 128),
                            ratatui::style::Color::Rgb(255, 144, 176),
                            ratatui::style::Color::Rgb(144, 255, 120),
                            ratatui::style::Color::Rgb(176, 144, 255),
                            ratatui::style::Color::Rgb(160, 208, 255),
                            ratatui::style::Color::Rgb(255, 144, 112),
                            ratatui::style::Color::Rgb(160, 200, 144),
                        ];
                        PALETTES[(hash as usize) % PALETTES.len()]
                    })
                    .collect();
                if !session_colors.is_empty() {
                    self.lava_lamp.set_session_colors(session_colors);
                }
            }

            // Rescan processes every 3 seconds
            if self.last_scan.elapsed() > Duration::from_secs(3) {
                self.detected_sessions = self.scanner.scan();
                self.enrich_sessions();
                self.refresh_session_data();
                self.enrich_from_jsonl();
                self.cleanup_stale_status_files(&statuses, &status_dir);
                self.last_scan = Instant::now();
            }

            // Build sessions: status files are the source of truth for Claude Code
            let paired = self.build_sessions(&statuses);

            // Track activity from ALL sessions
            self.collect_activity(&statuses);

            // Sample token history every 30 ticks (~2s at 15fps)
            if self.tick_count % 30 == 0 {
                self.record_token_history(&statuses);
            }

            let theme = &self.theme;
            let tick = self.tick_count;
            let feed = &self.feed_entries;
            let rain = &self.matrix_rain;
            let overlay_mode = self.overlay_mode;
            let overlay_scroll = self.overlay_scroll;
            let history_entries = &self.history_entries;
            let history_selected = self.history_selected;
            let history_delete_confirm = self.history_delete_confirm;
            let bands = self.audio_capture.bands.lock().unwrap().clone();
            let active_vis = self.active_visualizer;
            let vis_styles = self.visualizer_styles.clone();
            let vis_name = {
                let styles = vis_styles.lock().unwrap();
                if active_vis < styles.len() { styles[active_vis].name().to_string() } else { "---".to_string() }
            };
            let rain_visible = self.rain_visible;
            let vis_visible = self.visualizer_visible;
            let mem_ws = self.memory_watcher.world_state.lock().unwrap().clone();
            let mem_health = self.memory_watcher.health.lock().unwrap().clone();
            let memory_visible = self.memory_visible;
            let explorer_state = &self.explorer_state;
            let glow = &self.breathing_glow;
            let glow_visible = self.glow_visible;
            let fireflies = &self.fireflies;
            let fireflies_visible = self.fireflies_visible;
            let lava_lamp = &self.lava_lamp;
            let lava_visible = self.lava_visible;
            let fractal_plasma = &self.fractal_plasma;
            let plasma_visible = self.plasma_visible;
            let cosmic_eye = &self.cosmic_eye;
            let eye_visible = self.eye_visible;
            terminal.draw(|frame| {
                // Cosmic eye as bottom-most layer
                if eye_visible {
                    cosmic_eye.render(frame, frame.area(), theme, helix_state);
                }
                // Fractal plasma layer
                if plasma_visible {
                    fractal_plasma.render(frame, frame.area(), theme, helix_state);
                }
                // Lava lamp as background layer
                if lava_visible {
                    lava_lamp.render(frame, frame.area(), theme, helix_state);
                }
                // Rain as bottom layer — rendered first, everything else draws on top
                if rain_visible {
                    rain.render_overlay(frame, frame.area(), theme, helix_state);
                }
                // Fireflies as overlay on empty space
                if fireflies_visible {
                    fireflies.render(frame, frame.area(), theme, helix_state);
                }
                let (vis_area, _bottom_area) = crate::layout::render(frame, &paired, &primary_status, theme, tick, feed, rain, &vis_name, rain_visible, vis_visible, &mem_ws, &mem_health, memory_visible);
                // Render visualizer into the returned area
                if let Some(vis_rect) = vis_area {
                    if vis_visible {
                        let mut styles = vis_styles.lock().unwrap();
                        if active_vis < styles.len() {
                            styles[active_vis].render(frame, vis_rect, &bands, theme, tick);
                        }
                    }
                }
                match overlay_mode {
                    OverlayMode::Activity => {
                        crate::layout::render_activity_overlay(frame, feed, theme, overlay_scroll);
                    }
                    OverlayMode::History => {
                        crate::widgets::history_overlay::render_history_overlay(
                            frame, history_entries, history_selected, history_delete_confirm, theme,
                        );
                    }
                    OverlayMode::MemoryExplorer => {
                        let default_explorer = crate::memory_state::ExplorerData::default();
                        let explorer_data = mem_ws.as_ref().map(|w| &w.explorer).unwrap_or(&default_explorer);
                        crate::widgets::memory_explorer::render_memory_explorer(
                            frame,
                            explorer_data,
                            explorer_state,
                            theme,
                        );
                    }
                    OverlayMode::Finance => {
                        let default_explorer = crate::memory_state::ExplorerData::default();
                        let explorer_data = mem_ws.as_ref().map(|w| &w.explorer).unwrap_or(&default_explorer);
                        crate::widgets::finance_overlay::render_finance_overlay(
                            frame,
                            explorer_data,
                            &self.finance_state,
                            theme,
                        );
                    }
                    OverlayMode::None => {}
                }
                // Breathing glow modifies existing border cells — render last
                if glow_visible {
                    glow.render(frame, frame.area(), theme, helix_state);
                }
            })?;

            // Clamp overlay scroll to valid range (count LINES not entries)
            if self.overlay_mode == OverlayMode::Activity {
                let entries = self.feed_entries.lock().unwrap();
                let mut total_lines: u16 = 0;
                for entry in entries.iter() {
                    if entry.is_user_message {
                        total_lines += 2;
                    } else {
                        total_lines += 1; // main line
                        if !entry.detail.is_empty() || !entry.result.is_empty() {
                            total_lines += 1; // detail line
                        }
                        total_lines += 1; // blank separator
                    }
                }
                drop(entries);
                let visible = terminal.size()?.height.saturating_sub(4);
                let max_scroll = total_lines.saturating_sub(visible);
                if self.overlay_scroll > max_scroll {
                    self.overlay_scroll = max_scroll;
                }
            }

            let timeout = tick_rate.saturating_sub(tick_start.elapsed());
            if event::poll(timeout)? {
                let ev = event::read()?;

                // ── Mouse events ──
                if let Event::Mouse(mouse) = &ev {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            if self.overlay_mode == OverlayMode::Activity {
                                self.overlay_scroll = self.overlay_scroll.saturating_sub(3);
                            } else if self.overlay_mode == OverlayMode::MemoryExplorer
                                && !self.explorer_state.search_active
                                && !self.explorer_state.command_active
                                && !self.explorer_state.detail_open
                            {
                                let tab = self.explorer_state.active_tab;
                                self.explorer_state.tab_selected[tab] = self.explorer_state.tab_selected[tab].saturating_sub(1);
                            }
                        }
                        MouseEventKind::ScrollDown => {
                            if self.overlay_mode == OverlayMode::Activity {
                                self.overlay_scroll = self.overlay_scroll.saturating_add(3);
                            } else if self.overlay_mode == OverlayMode::MemoryExplorer
                                && !self.explorer_state.search_active
                                && !self.explorer_state.command_active
                                && !self.explorer_state.detail_open
                            {
                                let tab = self.explorer_state.active_tab;
                                self.explorer_state.tab_selected[tab] = self.explorer_state.tab_selected[tab].saturating_add(1);
                            }
                        }
                        _ => {}
                    }
                }

                if let Event::Key(key) = ev {
                    if key.kind == KeyEventKind::Press {
                        // ── Memory Explorer overlay: intercept ALL keys ──
                        if self.overlay_mode == OverlayMode::MemoryExplorer {
                            if self.explorer_state.search_active {
                                match key.code {
                                    KeyCode::Esc => {
                                        self.explorer_state.search_active = false;
                                    }
                                    KeyCode::Backspace => {
                                        self.explorer_state.search_query.pop();
                                    }
                                    KeyCode::Enter => {
                                        self.explorer_state.search_active = false;
                                    }
                                    KeyCode::Char(c) => {
                                        self.explorer_state.search_query.push(c);
                                    }
                                    _ => {}
                                }
                            } else if self.explorer_state.command_active {
                                match key.code {
                                    KeyCode::Esc => {
                                        self.explorer_state.command_active = false;
                                        self.explorer_state.command_input.clear();
                                    }
                                    KeyCode::Backspace => {
                                        self.explorer_state.command_input.pop();
                                    }
                                    KeyCode::Enter => {
                                        let cmd = self.explorer_state.command_input.clone();
                                        self.explorer_state.command_active = false;
                                        self.explorer_state.command_input.clear();
                                        self.execute_explorer_command(cmd);
                                    }
                                    KeyCode::Char(c) => {
                                        self.explorer_state.command_input.push(c);
                                    }
                                    _ => {}
                                }
                            } else if self.explorer_state.detail_open {
                                match key.code {
                                    KeyCode::Up | KeyCode::Char('k') => {
                                        self.explorer_state.detail_cursor = self.explorer_state.detail_cursor.saturating_sub(1);
                                    }
                                    KeyCode::Down | KeyCode::Char('j') => {
                                        self.explorer_state.detail_cursor = self.explorer_state.detail_cursor.saturating_add(1);
                                    }
                                    KeyCode::Enter => {
                                        // Chain navigate to selected relation link
                                        let mem_ws = self.memory_watcher.world_state.lock().unwrap().clone();
                                        if let Some(ref ws) = mem_ws {
                                            let entity = crate::widgets::memory_explorer::find_entity_by_name_pub(&ws.explorer, &self.explorer_state.detail_entity_id);
                                            if let Some(ent) = entity {
                                                let backlinks = crate::widgets::memory_explorer::find_backlinks_pub(&ws.explorer, &ent.0);
                                                let all_links: Vec<String> = backlinks.iter().map(|l| l.1.clone())
                                                    .chain(ent.1.iter().map(|l| l.1.clone()))
                                                    .collect();
                                                if let Some(target) = all_links.get(self.explorer_state.detail_cursor) {
                                                    // Only navigate if target entity actually exists
                                                    if crate::widgets::memory_explorer::find_entity_by_name_pub(&ws.explorer, target).is_some() {
                                                        let prev = self.explorer_state.detail_entity_id.clone();
                                                        // Cap history at 50 to prevent unbounded growth
                                                        if self.explorer_state.detail_history.len() >= 50 {
                                                            self.explorer_state.detail_history.remove(0);
                                                        }
                                                        self.explorer_state.detail_history.push(prev);
                                                        self.explorer_state.detail_entity_id = target.clone();
                                                        self.explorer_state.detail_cursor = 0;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    KeyCode::Backspace => {
                                        if let Some(prev) = self.explorer_state.detail_history.pop() {
                                            self.explorer_state.detail_entity_id = prev;
                                            self.explorer_state.detail_cursor = 0;
                                        } else {
                                            self.explorer_state.detail_open = false;
                                        }
                                    }
                                    KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => {
                                        self.explorer_state.detail_open = false;
                                        self.explorer_state.detail_cursor = 0;
                                        self.explorer_state.detail_history.clear();
                                    }
                                    KeyCode::Char('q') | KeyCode::Char('w') | KeyCode::Char('W') => {
                                        self.overlay_mode = OverlayMode::None;
                                        self.explorer_state.reset();
                                    }
                                    _ => {}
                                }
                            } else {
                                match key.code {
                                    KeyCode::Tab => {
                                        self.explorer_state.active_tab = (self.explorer_state.active_tab + 1) % 5;
                                        self.explorer_state.expanded.clear();
                                    }
                                    KeyCode::Right => {
                                        let tab = self.explorer_state.active_tab;
                                        let sel = self.explorer_state.tab_selected[tab];
                                        if self.explorer_state.expanded.contains(&sel) {
                                            let mem_ws = self.memory_watcher.world_state.lock().unwrap().clone();
                                            if let Some(ref ws) = mem_ws {
                                                let names = crate::widgets::memory_explorer::get_linked_names_pub(&ws.explorer, tab, sel, &self.explorer_state.search_query);
                                                if let Some(first) = names.first() {
                                                    self.explorer_state.detail_open = true;
                                                    self.explorer_state.detail_entity_id = first.clone();
                                                    self.explorer_state.detail_cursor = 0;
                                                    self.explorer_state.detail_history.clear();
                                                } else {
                                                    self.explorer_state.active_tab = (self.explorer_state.active_tab + 1) % 5;
                                                    self.explorer_state.expanded.clear();
                                                }
                                            } else {
                                                self.explorer_state.active_tab = (self.explorer_state.active_tab + 1) % 5;
                                                self.explorer_state.expanded.clear();
                                            }
                                        } else {
                                            self.explorer_state.active_tab = (self.explorer_state.active_tab + 1) % 5;
                                            self.explorer_state.expanded.clear();
                                        }
                                    }
                                    KeyCode::Left => {
                                        self.explorer_state.active_tab = if self.explorer_state.active_tab == 0 { 4 } else { self.explorer_state.active_tab - 1 };
                                        self.explorer_state.expanded.clear();
                                    }
                                    KeyCode::Up | KeyCode::Char('k') => {
                                        let tab = self.explorer_state.active_tab;
                                        self.explorer_state.tab_selected[tab] = self.explorer_state.tab_selected[tab].saturating_sub(1);
                                    }
                                    KeyCode::Down | KeyCode::Char('j') => {
                                        let tab = self.explorer_state.active_tab;
                                        self.explorer_state.tab_selected[tab] = self.explorer_state.tab_selected[tab].saturating_add(1);
                                    }
                                    KeyCode::PageUp => {
                                        let tab = self.explorer_state.active_tab;
                                        self.explorer_state.tab_selected[tab] = self.explorer_state.tab_selected[tab].saturating_sub(10);
                                    }
                                    KeyCode::PageDown => {
                                        let tab = self.explorer_state.active_tab;
                                        self.explorer_state.tab_selected[tab] = self.explorer_state.tab_selected[tab].saturating_add(10);
                                    }
                                    KeyCode::Enter => {
                                        let tab = self.explorer_state.active_tab;
                                        let sel = self.explorer_state.tab_selected[tab];
                                        if self.explorer_state.expanded.contains(&sel) {
                                            self.explorer_state.expanded.remove(&sel);
                                        } else {
                                            self.explorer_state.expanded.insert(sel);
                                        }
                                    }
                                    KeyCode::Char('l') => {
                                        let tab = self.explorer_state.active_tab;
                                        let sel = self.explorer_state.tab_selected[tab];
                                        if self.explorer_state.expanded.contains(&sel) {
                                            let mem_ws = self.memory_watcher.world_state.lock().unwrap().clone();
                                            if let Some(ref ws) = mem_ws {
                                                let names = crate::widgets::memory_explorer::get_linked_names_pub(&ws.explorer, tab, sel, &self.explorer_state.search_query);
                                                if let Some(first) = names.first() {
                                                    self.explorer_state.detail_open = true;
                                                    self.explorer_state.detail_entity_id = first.clone();
                                                    self.explorer_state.detail_cursor = 0;
                                                    self.explorer_state.detail_history.clear();
                                                }
                                            }
                                        }
                                    }
                                    KeyCode::Char('/') => {
                                        self.explorer_state.search_active = true;
                                    }
                                    KeyCode::Char(':') => {
                                        self.explorer_state.command_active = true;
                                        self.explorer_state.command_input.clear();
                                    }
                                    KeyCode::Char('d') => {
                                        // Mark selected action as done (Actions tab only)
                                        if self.explorer_state.active_tab == 4 {
                                            let mem_ws = self.memory_watcher.world_state.lock().unwrap().clone();
                                            if let Some(ref ws) = mem_ws {
                                                let sel = self.explorer_state.tab_selected[4];
                                                if let Some((name, _, _)) = crate::widgets::memory_explorer::get_selected_entity_info(
                                                    &ws.explorer, 4, sel, &self.explorer_state.search_query
                                                ) {
                                                    self.explorer_state.busy = true;
                                                    self.explorer_state.set_toast(format!("Completing: {}...", &name), ToastLevel::Info);
                                                    let tx = self.explorer_cmd_tx.clone();
                                                    tokio::spawn(async move {
                                                        let result = tokio::process::Command::new("hmem")
                                                            .args(["entity", "update", &name, "--status", "completed"])
                                                            .output().await;
                                                        let _ = tokio::process::Command::new("hmem")
                                                            .args(["refresh"])
                                                            .output().await;
                                                        let msg = match result {
                                                            Ok(o) if o.status.success() => ExplorerCmdResult {
                                                                success: true,
                                                                message: format!("Done: {}", name),
                                                            },
                                                            Ok(o) => ExplorerCmdResult {
                                                                success: false,
                                                                message: String::from_utf8_lossy(&o.stderr).trim().to_string(),
                                                            },
                                                            Err(e) => ExplorerCmdResult {
                                                                success: false,
                                                                message: e.to_string(),
                                                            },
                                                        };
                                                        let _ = tx.send(msg);
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    KeyCode::Char('p') => {
                                        // Cycle priority (Actions) or status (Projects)
                                        let mem_ws = self.memory_watcher.world_state.lock().unwrap().clone();
                                        if let Some(ref ws) = mem_ws {
                                            let tab = self.explorer_state.active_tab;
                                            let sel = self.explorer_state.tab_selected[tab];
                                            if let Some((name, etype, current)) = crate::widgets::memory_explorer::get_selected_entity_info(
                                                &ws.explorer, tab, sel, &self.explorer_state.search_query
                                            ) {
                                                let (flag, next_val) = if tab == 4 {
                                                    // Cycle priority: low → medium → high → critical → low
                                                    let next = match current.as_str() {
                                                        "low" => "medium",
                                                        "medium" => "high",
                                                        "high" => "critical",
                                                        "critical" => "low",
                                                        _ => "medium",
                                                    };
                                                    ("--priority", next.to_string())
                                                } else if tab == 0 {
                                                    // Cycle status: active → paused → completed → active
                                                    let next = match current.as_str() {
                                                        "active" => "paused",
                                                        "paused" => "completed",
                                                        "completed" => "active",
                                                        _ => "active",
                                                    };
                                                    ("--status", next.to_string())
                                                } else {
                                                    ("", String::new())
                                                };
                                                if !flag.is_empty() {
                                                    self.explorer_state.busy = true;
                                                    self.explorer_state.set_toast(format!("{} -> {}", name, next_val), ToastLevel::Info);
                                                    let tx = self.explorer_cmd_tx.clone();
                                                    let flag = flag.to_string();
                                                    tokio::spawn(async move {
                                                        let result = tokio::process::Command::new("hmem")
                                                            .args(["entity", "update", &name, &flag, &next_val])
                                                            .output().await;
                                                        let _ = tokio::process::Command::new("hmem")
                                                            .args(["refresh"])
                                                            .output().await;
                                                        let msg = match result {
                                                            Ok(o) if o.status.success() => ExplorerCmdResult {
                                                                success: true,
                                                                message: format!("{}: {}", name, next_val),
                                                            },
                                                            Ok(o) => ExplorerCmdResult {
                                                                success: false,
                                                                message: String::from_utf8_lossy(&o.stderr).trim().to_string(),
                                                            },
                                                            Err(e) => ExplorerCmdResult {
                                                                success: false,
                                                                message: e.to_string(),
                                                            },
                                                        };
                                                        let _ = tx.send(msg);
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    KeyCode::Char('$') => {
                                        // Switch from Memory Explorer to Finance overlay
                                        self.overlay_mode = OverlayMode::Finance;
                                        self.explorer_state.reset();
                                        self.finance_state.reset();
                                    }
                                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('w') | KeyCode::Char('W') => {
                                        self.overlay_mode = OverlayMode::None;
                                        self.explorer_state.reset();
                                    }
                                    _ => {}
                                }
                                // Clamp cursor to valid range
                                {
                                    let tab = self.explorer_state.active_tab;
                                    let mem_ws = self.memory_watcher.world_state.lock().unwrap();
                                    if let Some(ref ws) = *mem_ws {
                                        let count = match tab {
                                            0 => ws.explorer.projects.iter().filter(|p| {
                                                self.explorer_state.search_query.is_empty()
                                                || p.name.to_lowercase().contains(&self.explorer_state.search_query.to_lowercase())
                                                || p.context.to_lowercase().contains(&self.explorer_state.search_query.to_lowercase())
                                                || p.scope.to_lowercase().contains(&self.explorer_state.search_query.to_lowercase())
                                            }).count(),
                                            1 => ws.explorer.people.iter().filter(|p| {
                                                self.explorer_state.search_query.is_empty()
                                                || p.name.to_lowercase().contains(&self.explorer_state.search_query.to_lowercase())
                                                || p.role.to_lowercase().contains(&self.explorer_state.search_query.to_lowercase())
                                            }).count(),
                                            2 => ws.explorer.decisions.iter().filter(|d| {
                                                self.explorer_state.search_query.is_empty()
                                                || d.name.to_lowercase().contains(&self.explorer_state.search_query.to_lowercase())
                                                || d.summary.to_lowercase().contains(&self.explorer_state.search_query.to_lowercase())
                                            }).count(),
                                            3 => ws.explorer.timeline.iter().filter(|t| {
                                                self.explorer_state.search_query.is_empty()
                                                || t.summary.to_lowercase().contains(&self.explorer_state.search_query.to_lowercase())
                                                || t.source.to_lowercase().contains(&self.explorer_state.search_query.to_lowercase())
                                                || t.entity_names.iter().any(|n| n.to_lowercase().contains(&self.explorer_state.search_query.to_lowercase()))
                                            }).count(),
                                            4 => ws.explorer.actions.iter().filter(|a| {
                                                self.explorer_state.search_query.is_empty()
                                                || a.description.to_lowercase().contains(&self.explorer_state.search_query.to_lowercase())
                                                || a.project.to_lowercase().contains(&self.explorer_state.search_query.to_lowercase())
                                            }).count(),
                                            _ => 0,
                                        };
                                        if count > 0 {
                                            self.explorer_state.tab_selected[tab] = self.explorer_state.tab_selected[tab].min(count - 1);
                                        } else {
                                            self.explorer_state.tab_selected[tab] = 0;
                                        }
                                    }
                                }
                            }
                            continue;
                        }

                        // ── Finance overlay: intercept keys ──
                        if self.overlay_mode == OverlayMode::Finance {
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('q') => {
                                    self.overlay_mode = OverlayMode::None;
                                    self.finance_state.reset();
                                }
                                KeyCode::Char('w') | KeyCode::Char('W') => {
                                    // Go back to Memory Explorer
                                    self.overlay_mode = OverlayMode::MemoryExplorer;
                                    self.finance_state.reset();
                                    self.explorer_state.reset();
                                }
                                KeyCode::Tab => {
                                    self.finance_state.view = (self.finance_state.view + 1) % 2;
                                    self.finance_state.scroll = 0;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    self.finance_state.selected = self.finance_state.selected.saturating_sub(1);
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    let mem_ws = self.memory_watcher.world_state.lock().unwrap();
                                    let max = mem_ws.as_ref().map(|w| w.explorer.payments.len().saturating_sub(1)).unwrap_or(0);
                                    drop(mem_ws);
                                    if self.finance_state.selected < max {
                                        self.finance_state.selected += 1;
                                    }
                                }
                                KeyCode::Char('m') => {
                                    // Mark selected payment as paid
                                    let mem_ws = self.memory_watcher.world_state.lock().unwrap().clone();
                                    if let Some(ref ws) = mem_ws {
                                        if let Some(payment) = ws.explorer.payments.get(self.finance_state.selected) {
                                            if payment.status != "paid" {
                                                let name = payment.name.clone();
                                                self.explorer_state.busy = true;
                                                self.explorer_state.set_toast(format!("Marking paid: {}...", &name), ToastLevel::Info);
                                                let tx = self.explorer_cmd_tx.clone();
                                                tokio::spawn(async move {
                                                    let result = tokio::process::Command::new("hmem")
                                                        .args(["entity", "update", &name, "--status", "completed"])
                                                        .output().await;
                                                    let _ = tokio::process::Command::new("hmem")
                                                        .args(["refresh"])
                                                        .output().await;
                                                    let msg = match result {
                                                        Ok(o) if o.status.success() => ExplorerCmdResult {
                                                            success: true,
                                                            message: format!("Paid: {}", name),
                                                        },
                                                        Ok(o) => ExplorerCmdResult {
                                                            success: false,
                                                            message: String::from_utf8_lossy(&o.stderr).trim().to_string(),
                                                        },
                                                        Err(e) => ExplorerCmdResult {
                                                            success: false,
                                                            message: e.to_string(),
                                                        },
                                                    };
                                                    let _ = tx.send(msg);
                                                });
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                            continue;
                        }

                        // wallet_pending no longer used — finance opens from inside Memory Explorer via 'm'

                        // Reset delete confirmation on any non-D key in History mode
                        if self.overlay_mode == OverlayMode::History
                            && !matches!(key.code, KeyCode::Char('D') | KeyCode::Char('d'))
                        {
                            self.history_delete_confirm = false;
                        }

                        match key.code {
                            KeyCode::Char('q') => {
                                if self.overlay_mode != OverlayMode::None {
                                    self.overlay_mode = OverlayMode::None;
                                    self.history_delete_confirm = false;
                                } else {
                                    self.running = false;
                                }
                            }
                            KeyCode::Char('T') => {
                                let names = Theme::all_names();
                                let current = names.iter().position(|&n| n == self.theme.name).unwrap_or(0);
                                let next = (current + 1) % names.len();
                                self.theme = Theme::by_name(names[next]);
                            }
                            KeyCode::Char('A') | KeyCode::Char('a') => {
                                self.history_delete_confirm = false;
                                if self.overlay_mode == OverlayMode::Activity {
                                    self.overlay_mode = OverlayMode::None;
                                } else {
                                    self.overlay_mode = OverlayMode::Activity;
                                    // Start scrolled to the bottom — render_activity_overlay clamps
                                    self.overlay_scroll = u16::MAX / 2;
                                }
                            }
                            KeyCode::Char('H') | KeyCode::Char('h') => {
                                self.history_delete_confirm = false;
                                if self.overlay_mode == OverlayMode::History {
                                    self.overlay_mode = OverlayMode::None;
                                } else {
                                    self.overlay_mode = OverlayMode::History;
                                    self.history_entries = history::read_history();
                                    self.history_entries.sort_by(|a, b| b.ended_at.cmp(&a.ended_at));
                                    self.history_selected = 0;
                                }
                            }
                            KeyCode::Char('S') | KeyCode::Char('s') => {
                                // Snapshot all live claude-code sessions to history
                                // Dedup is handled inside snapshot_session_to_history (2 min window)
                                for s in &statuses {
                                    if s.cli == "claude-code" && !s.cwd.is_empty() {
                                        self.snapshot_session_to_history(s);
                                    }
                                }
                            }
                            KeyCode::Esc => {
                                if self.overlay_mode != OverlayMode::None {
                                    self.overlay_mode = OverlayMode::None;
                                    self.history_delete_confirm = false;
                                }
                            }
                            KeyCode::Up => {
                                if self.overlay_mode == OverlayMode::Activity && self.overlay_scroll > 0 {
                                    self.overlay_scroll = self.overlay_scroll.saturating_sub(3);
                                } else if self.overlay_mode == OverlayMode::History {
                                    self.history_selected = self.history_selected.saturating_sub(1);
                                }
                            }
                            KeyCode::Down => {
                                if self.overlay_mode == OverlayMode::Activity {
                                    self.overlay_scroll = self.overlay_scroll.saturating_add(3);
                                } else if self.overlay_mode == OverlayMode::History {
                                    if !self.history_entries.is_empty() {
                                        self.history_selected = (self.history_selected + 1).min(self.history_entries.len() - 1);
                                    }
                                }
                            }
                            KeyCode::PageUp => {
                                if self.overlay_mode == OverlayMode::Activity {
                                    self.overlay_scroll = self.overlay_scroll.saturating_sub(30);
                                } else if self.overlay_mode == OverlayMode::History {
                                    self.history_selected = self.history_selected.saturating_sub(10);
                                }
                            }
                            KeyCode::PageDown => {
                                if self.overlay_mode == OverlayMode::Activity {
                                    self.overlay_scroll = self.overlay_scroll.saturating_add(30);
                                } else if self.overlay_mode == OverlayMode::History {
                                    if !self.history_entries.is_empty() {
                                        self.history_selected = (self.history_selected + 10).min(self.history_entries.len() - 1);
                                    }
                                }
                            }
                            KeyCode::Char('v') => {
                                // Cycle visualizer style
                                let len = self.visualizer_styles.lock().unwrap().len();
                                if len > 0 {
                                    self.active_visualizer = (self.active_visualizer + 1) % len;
                                }
                            }
                            KeyCode::Char('V') => {
                                // Toggle visualizer visibility
                                self.visualizer_visible = !self.visualizer_visible;
                            }
                            KeyCode::Char('R') | KeyCode::Char('r') => {
                                // Toggle rain visibility
                                self.rain_visible = !self.rain_visible;
                            }
                            KeyCode::Char('G') | KeyCode::Char('g') => {
                                // Toggle breathing glow
                                self.glow_visible = !self.glow_visible;
                            }
                            KeyCode::Char('F') | KeyCode::Char('f') => {
                                // Toggle fireflies
                                self.fireflies_visible = !self.fireflies_visible;
                            }
                            KeyCode::Char('L') | KeyCode::Char('l') => {
                                // Toggle lava lamp
                                self.lava_visible = !self.lava_visible;
                            }
                            KeyCode::Char('P') | KeyCode::Char('p') => {
                                // Toggle fractal plasma
                                self.plasma_visible = !self.plasma_visible;
                            }
                            KeyCode::Char('E') | KeyCode::Char('e') => {
                                // Toggle cosmic eye
                                self.eye_visible = !self.eye_visible;
                            }
                            KeyCode::Char('W') | KeyCode::Char('w') => {
                                if self.overlay_mode == OverlayMode::MemoryExplorer {
                                    self.overlay_mode = OverlayMode::None;
                                    self.explorer_state.reset();
                                } else {
                                    self.overlay_mode = OverlayMode::MemoryExplorer;
                                    self.explorer_state.reset();
                                }
                            }
                            KeyCode::Char('M') | KeyCode::Char('m') => {
                                // Toggle memory panel
                                self.memory_visible = !self.memory_visible;
                            }
                            KeyCode::Char('D') | KeyCode::Char('d') => {
                                if self.overlay_mode == OverlayMode::History && !self.history_entries.is_empty() {
                                    if self.history_delete_confirm {
                                        self.history_entries.remove(self.history_selected);
                                        let _ = history::rewrite_history(&self.history_entries);
                                        if self.history_selected >= self.history_entries.len() && self.history_selected > 0 {
                                            self.history_selected -= 1;
                                        }
                                        self.history_delete_confirm = false;
                                    } else {
                                        self.history_delete_confirm = true;
                                    }
                                }
                            }
                            KeyCode::Enter => {
                                if self.overlay_mode == OverlayMode::History {
                                    if let Some(entry) = self.history_entries.get(self.history_selected) {
                                        let _ = history::spawn_resume(entry);
                                        self.overlay_mode = OverlayMode::None;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            while rx.try_recv().is_ok() {}
            while mem_rx.try_recv().is_ok() {}

            // Poll explorer command results
            if let Some(ref mut cmd_rx) = self.explorer_cmd_rx {
                while let Ok(result) = cmd_rx.try_recv() {
                    let level = if result.success { ToastLevel::Success } else { ToastLevel::Error };
                    self.explorer_state.set_toast(result.message, level);
                }
            }

            self.tick_count += 1;
        }

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), DisableMouseCapture, LeaveAlternateScreen)?;
        // Force exit — background tasks (audio capture, file watchers, spawned hmem calls)
        // keep the tokio runtime alive otherwise
        std::process::exit(0);
    }

    /// Execute a command from the : command bar.
    fn execute_explorer_command(&mut self, input: String) {
        let input = input.trim().to_string();
        if input.is_empty() { return; }

        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let cmd = parts[0];
        let arg = parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();

        if arg.is_empty() && cmd != "refresh" {
            self.explorer_state.set_toast(format!("Usage: {} <value>", cmd), ToastLevel::Error);
            return;
        }

        let tx = self.explorer_cmd_tx.clone();
        self.explorer_state.busy = true;

        match cmd {
            "task" => {
                self.explorer_state.set_toast(format!("Creating task: {}...", &arg), ToastLevel::Info);
                tokio::spawn(async move {
                    let result = tokio::process::Command::new("hmem")
                        .args(["entity", "create", "--type", "task", "--name", &arg])
                        .output().await;
                    let _ = tokio::process::Command::new("hmem").args(["refresh"]).output().await;
                    let msg = match result {
                        Ok(o) if o.status.success() => ExplorerCmdResult { success: true, message: format!("Task created: {}", arg) },
                        Ok(o) => ExplorerCmdResult { success: false, message: String::from_utf8_lossy(&o.stderr).trim().to_string() },
                        Err(e) => ExplorerCmdResult { success: false, message: e.to_string() },
                    };
                    let _ = tx.send(msg);
                });
            }
            "note" => {
                self.explorer_state.set_toast("Saving note...".to_string(), ToastLevel::Info);
                tokio::spawn(async move {
                    let result = tokio::process::Command::new("hmem")
                        .args(["remember", &arg, "--type", "note"])
                        .output().await;
                    let _ = tokio::process::Command::new("hmem").args(["refresh"]).output().await;
                    let msg = match result {
                        Ok(o) if o.status.success() => ExplorerCmdResult { success: true, message: "Note saved".to_string() },
                        Ok(o) => ExplorerCmdResult { success: false, message: String::from_utf8_lossy(&o.stderr).trim().to_string() },
                        Err(e) => ExplorerCmdResult { success: false, message: e.to_string() },
                    };
                    let _ = tx.send(msg);
                });
            }
            "done" => {
                self.explorer_state.set_toast(format!("Completing: {}...", &arg), ToastLevel::Info);
                tokio::spawn(async move {
                    let result = tokio::process::Command::new("hmem")
                        .args(["entity", "update", &arg, "--status", "completed"])
                        .output().await;
                    let _ = tokio::process::Command::new("hmem").args(["refresh"]).output().await;
                    let msg = match result {
                        Ok(o) if o.status.success() => ExplorerCmdResult { success: true, message: format!("Done: {}", arg) },
                        Ok(o) => ExplorerCmdResult { success: false, message: String::from_utf8_lossy(&o.stderr).trim().to_string() },
                        Err(e) => ExplorerCmdResult { success: false, message: e.to_string() },
                    };
                    let _ = tx.send(msg);
                });
            }
            "refresh" => {
                self.explorer_state.set_toast("Refreshing...".to_string(), ToastLevel::Info);
                tokio::spawn(async move {
                    let _ = tokio::process::Command::new("hmem").args(["refresh"]).output().await;
                    let _ = tx.send(ExplorerCmdResult { success: true, message: "Refreshed".to_string() });
                });
            }
            _ => {
                self.explorer_state.set_toast(format!("Unknown: {}. Try: task, note, done, refresh", cmd), ToastLevel::Error);
                self.explorer_state.busy = false;
            }
        }
    }

    /// Build session list using status files as the primary source for Claude Code.
    /// Scanner can't get cwd on Windows, so status files (written by hooks/statusline)
    /// ARE the authoritative session data. Scanner provides memory info only.
    fn build_sessions(&self, statuses: &[SessionStatus]) -> Vec<(DetectedSession, Option<SessionStatus>)> {
        let mut results: Vec<(DetectedSession, Option<SessionStatus>)> = Vec::new();
        let mut used_cli_types: std::collections::HashSet<String> = std::collections::HashSet::new();

        // For each status file with cli=claude-code, create a session entry.
        // The status file IS the session — it has cwd, model, tokens, state.
        let mut claude_statuses: Vec<&SessionStatus> = statuses.iter()
            .filter(|s| s.cli == "claude-code" && !s.cwd.is_empty())
            .collect();
        claude_statuses.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Count how many claude.exe processes the scanner found
        let claude_process_count = self.detected_sessions.iter()
            .filter(|s| s.cli == "claude-code")
            .count();

        // Use scanner processes for memory info, distribute by index
        let claude_processes: Vec<&DetectedSession> = self.detected_sessions.iter()
            .filter(|s| s.cli == "claude-code")
            .collect();

        for (i, cs) in claude_statuses.iter().enumerate() {
            // Only show as many status-file sessions as there are actual processes
            // (prevents showing stale sessions after a CLI exits)
            if i >= claude_process_count && claude_process_count > 0 {
                break;
            }

            let memory = claude_processes.get(i).map(|p| p.memory).unwrap_or(0);
            let session = DetectedSession {
                pid: 0,
                cli: "claude-code".to_string(),
                cwd: cs.cwd.clone(),
                cpu: 0.0,
                memory,
            };
            results.push((session, Some((*cs).clone())));
        }

        used_cli_types.insert("claude-code".to_string());

        // For non-Claude CLIs (codex, gemini, aider), use the scanner as before
        for session in &self.detected_sessions {
            if used_cli_types.contains(&session.cli) {
                continue;
            }

            let matched = statuses.iter()
                .filter(|s| s.cli == session.cli && !s.cwd.is_empty())
                .max_by_key(|s| s.timestamp)
                .cloned();

            results.push((session.clone(), matched));
        }

        results
    }

    /// Get the most relevant status for global state (matrix rain, header)
    fn primary_status(&self, statuses: &[SessionStatus]) -> SessionStatus {
        let active = statuses.iter()
            .filter(|s| !s.state.is_empty() && s.state != "idle")
            .max_by_key(|s| s.timestamp);

        if let Some(s) = active {
            return s.clone();
        }

        statuses.iter()
            .max_by_key(|s| s.timestamp)
            .cloned()
            .unwrap_or_default()
    }

    /// Clean up stale status files. Only remove claude-code status files
    /// when there are more files than running claude.exe processes AND
    /// the file hasn't been updated in >5 minutes.
    fn cleanup_stale_status_files(&mut self, statuses: &[SessionStatus], status_dir: &std::path::Path) {
        let claude_process_count = self.detected_sessions.iter()
            .filter(|s| s.cli == "claude-code")
            .count();

        let mut claude_statuses: Vec<&SessionStatus> = statuses.iter()
            .filter(|s| s.cli == "claude-code" && !s.cwd.is_empty())
            .collect();

        // Only clean up if we have more status files than processes
        if claude_statuses.len() <= claude_process_count {
            return;
        }

        // Sort oldest first — remove the oldest excess files
        claude_statuses.sort_by_key(|s| s.timestamp);

        let excess = claude_statuses.len() - claude_process_count;
        let now = chrono::Utc::now().timestamp() as u64;

        for s in claude_statuses.iter().take(excess) {
            // Only remove if stale (>5 min without update)
            if now.saturating_sub(s.timestamp) > 300 {
                self.snapshot_session_to_history(s);
                let path = status_dir.join(status::status_filename(&s.cwd));
                let _ = std::fs::remove_file(path);
            }
        }
    }

    /// Snapshot a session's state to history.jsonl before cleanup
    fn snapshot_session_to_history(&mut self, status: &SessionStatus) {
        // Replace any existing entry for the same cwd (keeps history clean, no duplicates)
        let mut existing = history::read_history();
        let had_existing = existing.len();
        existing.retain(|e| e.cwd != status.cwd);
        if existing.len() != had_existing {
            // Rewrote without the old entry, we'll append the fresh one below
            let _ = history::rewrite_history(&existing);
        }

        let started_at_epoch = status.timestamp.saturating_sub(status.session.duration_ms / 1000);
        let started_at = chrono::DateTime::from_timestamp(started_at_epoch as i64, 0)
            .unwrap_or_default()
            .to_rfc3339();
        let ended_at = chrono::DateTime::from_timestamp(status.timestamp as i64, 0)
            .unwrap_or_default()
            .to_rfc3339();

        let hash = status::cwd_hash(&status.cwd);
        let id = format!("cwd-{}-{}", hash, started_at_epoch);

        // Collect up to 20 activity snapshots for this cwd
        let activities = {
            let entries = self.feed_entries.lock().unwrap();
            entries
                .iter()
                .filter(|e| !e.is_user_message && e.cwd == status.cwd)
                .rev()
                .take(20)
                .map(|e| history::ActivitySnapshot {
                    time: e.time.clone(),
                    tool: e.tool.clone(),
                    file: e.file.clone(),
                    description: e.description.clone(),
                    status: match e.status {
                        EntryStatus::Success => "success".to_string(),
                        EntryStatus::Failure => "failure".to_string(),
                        _ => "neutral".to_string(),
                    },
                })
                .collect::<Vec<_>>()
        };

        // Collect files touched for this session
        let status_key = status::status_filename(&status.cwd);
        let files: Vec<String> = self
            .files_touched
            .remove(&status_key)
            .map(|set| set.into_iter().collect())
            .unwrap_or_default();

        let entry = history::HistoryEntry {
            id,
            cwd: status.cwd.clone(),
            cli: status.cli.clone(),
            model: status.model.clone(),
            git_branch: status.git.branch.clone(),
            started_at,
            ended_at,
            duration_ms: status.session.duration_ms,
            tokens_in: status.tokens.input,
            tokens_out: status.tokens.output,
            context_used_pct: status.used_pct(),
            last_state: status.state.clone(),
            last_tool: status.activity.last_tool.clone(),
            last_file: status.activity.last_file.clone(),
            last_description: status.activity.last_description.clone(),
            activities,
            files_touched: files,
            session_id: history::find_claude_session_id(&status.cwd),
        };

        let _ = history::append_history(&entry);
    }

    /// Record token usage snapshots for sparkline display
    fn record_token_history(&mut self, statuses: &[SessionStatus]) {
        for s in statuses {
            if s.cli != "claude-code" || s.cwd.is_empty() {
                continue;
            }
            let pct = s.used_pct();
            if pct == 0 && s.tokens.context_size == 0 {
                continue; // no data yet
            }
            let key = status::normalize_cwd(&s.cwd);
            let history = self.token_history.entry(key).or_default();
            history.push(pct);
            if history.len() > 20 {
                history.remove(0);
            }
        }
    }

    /// Push new activity entries from all sessions into the feed
    fn collect_activity(&mut self, statuses: &[SessionStatus]) {
        let mut files_to_track: Vec<(String, String)> = Vec::new();
        {
        let mut entries = self.feed_entries.lock().unwrap();

        for s in statuses {
            if s.activity.last_tool.is_empty() || s.cwd.is_empty() {
                continue;
            }
            // Filter hook noise — only exact hook/statusline scripts, not project files
            if s.activity.last_file.contains("helix-post-tool")
                || s.activity.last_file.contains("helix-status.sh")
                || s.activity.last_file.contains("statusline-helix")
                || s.activity.last_file.contains(".ai-status")
            {
                continue;
            }

            // Dedup key: tool + file only (NOT timestamp — statusline updates timestamp
            // without changing tool/file, which would create false duplicates)
            let key = format!("{}|{}", s.activity.last_tool, s.activity.last_file);
            let cwd_norm = crate::status::normalize_cwd(&s.cwd);

            if self.last_activity_keys.get(&cwd_norm).map_or(true, |k| k != &key) {
                self.last_activity_keys.insert(cwd_norm, key);

                let now_ms = chrono::Utc::now().timestamp_millis() as u64;

                // Update previous entry's duration
                if !entries.is_empty() {
                    let prev_idx = entries.len() - 1;
                    if entries[prev_idx].timestamp_ms > 0 {
                        entries[prev_idx].duration_ms = now_ms.saturating_sub(entries[prev_idx].timestamp_ms);
                    }
                }

                // Determine status from hook-provided success flag
                let entry_status = match s.activity.last_success {
                    Some(true) => EntryStatus::Success,
                    Some(false) => EntryStatus::Failure,
                    None => EntryStatus::Pending,
                };

                entries.push(FeedEntry {
                    time: chrono::Local::now().format("%H:%M:%S").to_string(),
                    timestamp_ms: now_ms,
                    tool: s.activity.last_tool.clone(),
                    file: s.activity.last_file.clone(),
                    cwd: s.cwd.clone(),
                    description: s.activity.last_description.clone(),
                    detail: s.activity.last_detail.clone(),
                    result: s.activity.last_result.clone(),
                    duration_ms: 0,
                    status: entry_status,
                    is_user_message: false,
                    group_key: String::new(),
                });
                if entries.len() > 50 {
                    let drain_to = entries.len() - 50;
                    entries.drain(0..drain_to);
                }

                // Track files touched by Edit/Write tools
                let tool = s.activity.last_tool.as_str();
                if matches!(tool, "Edit" | "Write") && !s.activity.last_file.is_empty() {
                    let status_key = crate::status::status_filename(&s.cwd);
                    files_to_track.push((status_key, s.activity.last_file.clone()));
                }
            }
        }
        } // lock dropped
        for (key, file) in files_to_track {
            self.files_touched.entry(key).or_default().insert(file);
        }
    }

    /// Enrich detected sessions with data from CLI-specific session files
    fn enrich_sessions(&mut self) {
        let home = dirs::home_dir().unwrap_or_default();

        for session in &mut self.detected_sessions {
            if !session.cwd.is_empty() {
                continue;
            }

            match session.cli.as_str() {
                "codex" => {
                    if let Some((cwd, _model)) = read_codex_session(&home.join(".codex").join("sessions")) {
                        session.cwd = cwd;
                    }
                }
                "gemini" => {
                    if let Some(cwd) = read_gemini_cwd(&home.join(".gemini").join("history")) {
                        session.cwd = cwd;
                    }
                }
                _ => {}
            }
        }
    }

    fn enrich_from_jsonl(&mut self) {
        let claude_dir = dirs::home_dir().unwrap_or_default().join(".claude");
        let results = sessions::read_recent_tool_results(&claude_dir);
        if results.is_empty() { return; }

        let mut entries = self.feed_entries.lock().unwrap();
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;

        let mut result_idx = 0;
        for entry in entries.iter_mut() {
            if entry.is_user_message { continue; }
            if matches!(entry.status, EntryStatus::Pending) {
                while result_idx < results.len() {
                    if results[result_idx].tool == entry.tool {
                        entry.result = results[result_idx].result_summary.clone();
                        entry.status = if results[result_idx].success {
                            EntryStatus::Success
                        } else if entry.tool == "Edit" || entry.tool == "Read" {
                            EntryStatus::Warning
                        } else {
                            EntryStatus::Failure
                        };
                        result_idx += 1;
                        break;
                    }
                    result_idx += 1;
                }
                // Timeout: pending >30s → neutral
                if matches!(entry.status, EntryStatus::Pending) && entry.timestamp_ms > 0 {
                    if now_ms.saturating_sub(entry.timestamp_ms) > 30_000 {
                        entry.status = EntryStatus::Neutral;
                    }
                }
            }
        }
    }

    fn refresh_session_data(&mut self) {
        let claude_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".claude");
        self.session_data = sessions::find_active_session(&claude_dir);
    }
}

/// Read latest Codex session file for cwd and model
fn read_codex_session(sessions_dir: &std::path::Path) -> Option<(String, String)> {
    let mut newest: Option<(std::path::PathBuf, std::time::SystemTime)> = None;

    fn walk_dir(dir: &std::path::Path, newest: &mut Option<(std::path::PathBuf, std::time::SystemTime)>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk_dir(&path, newest);
                } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    if let Ok(meta) = path.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if newest.as_ref().map_or(true, |(_, t)| modified > *t) {
                                *newest = Some((path, modified));
                            }
                        }
                    }
                }
            }
        }
    }

    walk_dir(sessions_dir, &mut newest);

    let (path, _) = newest?;
    let content = std::fs::read_to_string(&path).ok()?;
    let first_line = content.lines().next()?;
    let data: serde_json::Value = serde_json::from_str(first_line).ok()?;
    let payload = data.get("payload")?;
    let cwd = payload.get("cwd")?.as_str()?.to_string();
    let model = payload.get("model_provider").and_then(|v| v.as_str()).unwrap_or("openai").to_string();
    Some((cwd, model))
}

/// Read Gemini CLI project root from history
fn read_gemini_cwd(history_dir: &std::path::Path) -> Option<String> {
    if !history_dir.exists() {
        return None;
    }

    let mut newest: Option<(String, std::time::SystemTime)> = None;

    if let Ok(entries) = std::fs::read_dir(history_dir) {
        for entry in entries.flatten() {
            let root_file = entry.path().join(".project_root");
            if root_file.exists() {
                if let Ok(meta) = root_file.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if newest.as_ref().map_or(true, |(_, t)| modified > *t) {
                            if let Ok(content) = std::fs::read_to_string(&root_file) {
                                let cwd = content.trim().to_string();
                                if !cwd.is_empty() {
                                    newest = Some((cwd, modified));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    newest.map(|(cwd, _)| cwd)
}
