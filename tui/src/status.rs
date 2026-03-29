use notify::{Watcher, RecursiveMode, Event, EventKind};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SessionStatus {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub cli: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub tokens: TokenStatus,
    #[serde(default)]
    pub session: SessionInfo,
    #[serde(default)]
    pub activity: ActivityInfo,
    #[serde(default)]
    pub git: GitInfo,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub timestamp: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TokenStatus {
    #[serde(default)]
    pub input: u64,
    #[serde(default)]
    pub output: u64,
    #[serde(default)]
    pub cache_read: u64,
    #[serde(default)]
    pub cache_write: u64,
    #[serde(default)]
    pub context_size: u64,
    #[serde(default)]
    pub used_pct: u32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SessionInfo {
    #[serde(default)]
    pub start_time: u64,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub api_duration_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ActivityInfo {
    #[serde(default)]
    pub last_tool: String,
    #[serde(default)]
    pub last_file: String,
    #[serde(default)]
    pub lines_added: u64,
    #[serde(default)]
    pub lines_removed: u64,
    #[serde(default)]
    pub last_description: String,
    #[serde(default)]
    pub last_detail: String,
    #[serde(default)]
    pub last_result: String,
    #[serde(default)]
    pub last_success: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GitInfo {
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub modified: u32,
    #[serde(default)]
    pub staged: u32,
}

/// Normalize a cwd path for consistent matching:
/// - backslashes → forward slashes
/// - lowercase
/// - convert Git Bash paths (/c/foo) → drive paths (c:/foo)
/// - strip trailing slashes
pub fn normalize_cwd(cwd: &str) -> String {
    let mut s = cwd.replace('\\', "/").to_lowercase();
    // Convert /c/... → c:/... (Git Bash style → Windows style)
    if s.len() >= 3 && s.starts_with('/') && s.as_bytes()[2] == b'/' {
        let drive = s.as_bytes()[1] as char;
        if drive.is_ascii_alphabetic() {
            s = format!("{}:/{}", drive, &s[3..]);
        }
    }
    while s.ends_with('/') {
        s.pop();
    }
    s
}

/// Hash a cwd into a short hex string for use as a filename key.
/// Uses FNV-1a (32-bit) which is simple enough to implement identically in Python.
pub fn cwd_hash(cwd: &str) -> String {
    let normalized = normalize_cwd(cwd);
    let hash = fnv1a_32(normalized.as_bytes());
    format!("{:08x}", hash)
}

/// FNV-1a 32-bit hash — deterministic, simple, reproducible in any language
pub fn fnv1a_32(data: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for &byte in data {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

/// Return the status filename prefix for a given cwd (matches all instances)
pub fn status_filename(cwd: &str) -> String {
    format!("cwd-{}.json", cwd_hash(cwd))
}

/// Return filename with instance ID for per-instance tracking
pub fn status_filename_instance(cwd: &str, instance_id: u32) -> String {
    format!("cwd-{}-{}.json", cwd_hash(cwd), instance_id)
}

/// Return the status filename prefix for matching per-instance files
/// Matches both `cwd-HASH.json` (legacy) and `cwd-HASH-PID.json` (new)
pub fn status_filename_prefix(cwd: &str) -> String {
    format!("cwd-{}", cwd_hash(cwd))
}

impl SessionStatus {
    pub fn used_pct(&self) -> u32 {
        if self.tokens.used_pct > 0 {
            return self.tokens.used_pct;
        }
        if self.tokens.context_size == 0 {
            return 0;
        }
        let total = self.tokens.input + self.tokens.output;
        ((total as f64 / self.tokens.context_size as f64) * 100.0) as u32
    }

    pub fn helix_state(&self) -> HelixState {
        // Streaming = tokens increasing = Claude is generating/thinking
        // Hook sets coding/reviewing/etc on tool use, which takes priority via timestamp
        if matches!(self.state.as_str(), "streaming" | "receiving" | "sending") {
            let now = chrono::Utc::now().timestamp() as u64;
            if self.timestamp > 0 && now.saturating_sub(self.timestamp) < 5 {
                return HelixState::Thinking;
            }
            return HelixState::Idle;
        }
        let pct = self.used_pct();
        if pct >= 90 {
            return HelixState::Critical;
        }
        if pct >= 70 {
            return HelixState::Deep;
        }
        match self.state.as_str() {
            "thinking" => HelixState::Thinking,
            "coding" | "editing" => HelixState::Coding,
            "reviewing" | "reading" => HelixState::Reviewing,
            "committing" => HelixState::Committing,
            "error" => HelixState::Error,
            "done" | "finished" => HelixState::Done,
            _ => HelixState::Idle,
        }
    }

    pub fn load_from_file(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelixState {
    Idle,
    Thinking,
    Coding,
    Reviewing,
    Committing,
    Streaming,
    Error,
    Done,
    Deep,
    Critical,
}

/// Watches the ~/.ai-status/ directory for per-session status files.
/// Returns all loaded statuses as a Vec.
pub struct StatusWatcher {
    dir: PathBuf,
    pub statuses: Arc<Mutex<Vec<SessionStatus>>>,
}

impl StatusWatcher {
    pub fn new(dir: PathBuf) -> Self {
        let statuses = Arc::new(Mutex::new(Vec::new()));
        // Initial load
        *statuses.lock().unwrap() = load_all_statuses(&dir);
        Self { dir, statuses }
    }

    pub async fn watch(&self, tx: mpsc::Sender<()>, debounce_ms: u64) {
        let dir = self.dir.clone();
        let statuses = self.statuses.clone();

        tokio::task::spawn_blocking(move || {
            let (notify_tx, notify_rx) = std::sync::mpsc::channel();
            let mut watcher = match notify::recommended_watcher(move |res: Result<Event, _>| {
                if let Ok(event) = res {
                    if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)) {
                        let _ = notify_tx.send(());
                    }
                }
            }) {
                Ok(w) => w,
                Err(_) => return,
            };

            if watcher.watch(&dir, RecursiveMode::NonRecursive).is_err() {
                return;
            }

            loop {
                if notify_rx.recv_timeout(Duration::from_secs(5)).is_ok() {
                    std::thread::sleep(Duration::from_millis(debounce_ms));
                    while notify_rx.try_recv().is_ok() {}

                    *statuses.lock().unwrap() = load_all_statuses(&dir);
                    let _ = tx.blocking_send(());
                }
            }
        });
    }
}

/// Load all status JSON files from the directory
fn load_all_statuses(dir: &Path) -> Vec<SessionStatus> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(status) = SessionStatus::load_from_file(&path) {
                    result.push(status);
                }
            }
        }
    }
    result
}
