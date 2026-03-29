use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub layout: LayoutConfig,
    #[serde(default)]
    pub widgets: WidgetConfigs,
}

#[derive(Debug, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_fps")]
    pub fps: u32,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_status_file")]
    pub status_file: String,
    #[serde(default = "default_debounce")]
    pub debounce_ms: u64,
}

#[derive(Debug, Deserialize, Default)]
pub struct LayoutConfig {
    #[serde(default = "default_columns")]
    pub columns: u16,
    #[serde(default = "default_rows")]
    pub rows: u16,
}

#[derive(Debug, Deserialize, Default)]
pub struct WidgetConfigs {
    #[serde(default)]
    pub context_gauge: WidgetToggle,
    #[serde(default)]
    pub activity_feed: WidgetToggle,
    #[serde(default)]
    pub session_timer: WidgetToggle,
    #[serde(default)]
    pub system_monitor: WidgetToggle,
    #[serde(default)]
    pub matrix_rain: WidgetToggle,
}

#[derive(Debug, Deserialize)]
pub struct WidgetToggle {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for WidgetToggle {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn default_mode() -> String { "dashboard".into() }
fn default_fps() -> u32 { 15 }
fn default_theme() -> String { "cyberpunk".into() }
fn default_status_file() -> String { "~/.ai-status.json".into() }
fn default_debounce() -> u64 { 100 }
fn default_columns() -> u16 { 2 }
fn default_rows() -> u16 { 4 }
fn default_true() -> bool { true }

impl Default for Config {
    fn default() -> Self {
        toml::from_str("").unwrap()
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            fps: default_fps(),
            theme: default_theme(),
            status_file: default_status_file(),
            debounce_ms: default_debounce(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
