use notify::{Watcher, RecursiveMode, Event, EventKind};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Structs — world_state_current.json (file_schema_version: 1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MemoryWorldState {
    #[serde(default)]
    pub file_schema_version: u32,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub stale_after: String,
    #[serde(default)]
    pub checksum: String,
    #[serde(default)]
    pub entity_count: u32,
    #[serde(default)]
    pub interaction_count: u32,
    #[serde(default)]
    pub pending_action_count: u32,
    #[serde(default)]
    pub document: String,
    #[serde(default)]
    pub sections: MemorySections,
    #[serde(default)]
    pub generation_duration_ms: u32,
    #[serde(default)]
    pub explorer: ExplorerData,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MemorySections {
    #[serde(default)]
    pub urgent: Vec<UrgentItem>,
    #[serde(default)]
    pub projects_aeos: Vec<ProjectSummary>,
    #[serde(default)]
    pub projects_metalfinger: Vec<ProjectSummary>,
    #[serde(default)]
    pub projects_personal: Vec<ProjectSummary>,
    #[serde(default)]
    pub waiting_on: Vec<WaitingItem>,
    #[serde(default)]
    pub deadlines: Vec<DeadlineItem>,
    #[serde(default)]
    pub pending_actions: Vec<ActionSummary>,
    #[serde(default)]
    pub stale_threads: Vec<StaleItem>,
    #[serde(default)]
    pub team_pulse: Vec<String>,
    #[serde(default)]
    pub recent_decisions: Vec<DecisionSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UrgentItem {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub source_entity: String,
    pub deadline: Option<String>,
    #[serde(default = "default_importance")]
    pub importance: f64,
}

fn default_importance() -> f64 {
    0.9
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSummary {
    #[serde(default)]
    pub entity_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status_line: String,
    pub deadline: Option<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub key_people: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WaitingItem {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub from_person: String,
    #[serde(default)]
    pub since: String,
    #[serde(default)]
    pub days_waiting: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeadlineItem {
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub days_left: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActionSummary {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created_at: String,
    pub deadline: Option<String>,
    #[serde(default)]
    pub age_days: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StaleItem {
    #[serde(default)]
    pub entity_name: String,
    #[serde(default)]
    pub last_activity: String,
    #[serde(default)]
    pub days_stale: u32,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DecisionSummary {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub date: Option<String>,
}

// ---------------------------------------------------------------------------
// Structs — explorer data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExplorerInteraction {
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub entity_names: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExplorerProjectCard {
    #[serde(default)]
    pub entity_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub context: String,
    pub deadline: Option<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub key_people: Vec<String>,
    #[serde(default)]
    pub pending_actions: Vec<String>,
    #[serde(default)]
    pub tasks: Vec<String>,
    #[serde(default)]
    pub recent_interactions: Vec<ExplorerInteraction>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExplorerPersonCard {
    #[serde(default)]
    pub entity_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub context: String,
    pub last_contact: Option<String>,
    #[serde(default)]
    pub projects: Vec<String>,
    #[serde(default)]
    pub recent_interactions: Vec<ExplorerInteraction>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExplorerDecisionCard {
    #[serde(default)]
    pub entity_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub summary: String,
    pub date: Option<String>,
    #[serde(default)]
    pub related_entities: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExplorerActionCard {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub project: String,
    pub deadline: Option<String>,
    #[serde(default)]
    pub age_days: u32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExplorerTimelineEntry {
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub entity_names: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExplorerPaymentCard {
    #[serde(default)]
    pub entity_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub amount: i64,
    #[serde(default)]
    pub currency: String,
    pub due_date: Option<String>,
    pub paid_date: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub is_salary: bool,
    #[serde(default)]
    pub is_recurring: bool,
    pub days_left: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MonthForecast {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub year: i32,
    #[serde(default)]
    pub month: i32,
    #[serde(default)]
    pub salary: i64,
    #[serde(default)]
    pub freelance: i64,
    #[serde(default)]
    pub freelance_confirmed: i64,
    #[serde(default)]
    pub freelance_gap: i64,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub received: i64,
    #[serde(default)]
    pub freelance_cumulative: i64,
    #[serde(default)]
    pub freelance_running_avg: i64,
    #[serde(default)]
    pub on_target: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FinanceSummary {
    #[serde(default)]
    pub monthly_received: i64,
    #[serde(default)]
    pub monthly_pending: i64,
    #[serde(default)]
    pub monthly_overdue: i64,
    #[serde(default)]
    pub monthly_owed: i64,
    #[serde(default)]
    pub monthly_expected: i64,
    #[serde(default)]
    pub yearly_received: i64,
    #[serde(default)]
    pub yearly_expected: i64,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub salary_amount: i64,
    #[serde(default)]
    pub salary_source: String,
    #[serde(default)]
    pub freelance_target: i64,
    #[serde(default)]
    pub freelance_target_yearly: i64,
    #[serde(default)]
    pub freelance_earned: i64,
    #[serde(default)]
    pub freelance_pipeline: i64,
    #[serde(default)]
    pub freelance_locked: i64,
    #[serde(default)]
    pub freelance_target_cumulative: i64,
    #[serde(default)]
    pub freelance_ahead_behind: i64,
    #[serde(default)]
    pub freelance_runway_months: f64,
    #[serde(default)]
    pub freelance_required_rate: i64,
    #[serde(default)]
    pub freelance_completion_pct: f64,
    #[serde(default)]
    pub freelance_months_elapsed: i32,
    #[serde(default)]
    pub freelance_months_remaining: i32,
    #[serde(default)]
    pub freelance_ytd: i64,
    #[serde(default)]
    pub freelance_avg: i64,
    #[serde(default)]
    pub freelance_months_counted: i32,
    #[serde(default)]
    pub forecast: Vec<MonthForecast>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExplorerData {
    #[serde(default)]
    pub projects: Vec<ExplorerProjectCard>,
    #[serde(default)]
    pub people: Vec<ExplorerPersonCard>,
    #[serde(default)]
    pub decisions: Vec<ExplorerDecisionCard>,
    #[serde(default)]
    pub actions: Vec<ExplorerActionCard>,
    #[serde(default)]
    pub timeline: Vec<ExplorerTimelineEntry>,
    #[serde(default)]
    pub payments: Vec<ExplorerPaymentCard>,
    #[serde(default)]
    pub finance_summary: FinanceSummary,
}

// ---------------------------------------------------------------------------
// Structs — health.json
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MemoryHealth {
    #[serde(default)]
    pub file_schema_version: u32,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub qdrant_connected: bool,
    #[serde(default)]
    pub entity_count: u32,
    #[serde(default)]
    pub interaction_count: u32,
    #[serde(default)]
    pub last_compaction: String,
    #[serde(default)]
    pub checked_at: String,
}

// ---------------------------------------------------------------------------
// Helper methods
// ---------------------------------------------------------------------------

impl MemoryWorldState {
    /// Returns true if the world state data is stale (past stale_after time).
    /// Parses stale_after as an RFC 3339 datetime and compares with now.
    /// Returns true on parse failure (treat unknown staleness as stale).
    pub fn is_stale(&self) -> bool {
        chrono::DateTime::parse_from_rfc3339(&self.stale_after)
            .map(|stale_time| chrono::Utc::now() > stale_time)
            .unwrap_or(true)
    }
}

// ---------------------------------------------------------------------------
// MemoryWatcher — follows the exact same pattern as StatusWatcher in status.rs
// ---------------------------------------------------------------------------

/// Watches ~/.helix/state/ for world_state_current.json and health.json.
pub struct MemoryWatcher {
    dir: PathBuf,
    pub world_state: Arc<Mutex<Option<MemoryWorldState>>>,
    pub health: Arc<Mutex<Option<MemoryHealth>>>,
}

impl MemoryWatcher {
    pub fn new() -> Self {
        let dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".helix")
            .join("state");
        Self {
            dir,
            world_state: Arc::new(Mutex::new(None)),
            health: Arc::new(Mutex::new(None)),
        }
    }

    /// Load current state from files. Call once at startup before spawning watch().
    pub fn load_initial(&self) {
        self.reload();
    }

    /// Watch ~/.helix/state/ for file changes. Runs forever inside a blocking
    /// task, matching the exact debounce + notify pattern used by StatusWatcher.
    pub async fn watch(&self, tx: mpsc::Sender<()>, debounce_ms: u64) {
        let dir = self.dir.clone();
        let world_state = self.world_state.clone();
        let health = self.health.clone();

        tokio::task::spawn_blocking(move || {
            let (notify_tx, notify_rx) = std::sync::mpsc::channel();
            let mut watcher = match notify::recommended_watcher(move |res: Result<Event, _>| {
                if let Ok(event) = res {
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    ) {
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
                    // Drain any additional events that arrived during the debounce window
                    while notify_rx.try_recv().is_ok() {}

                    Self::reload_into(&dir, &world_state, &health);
                    let _ = tx.blocking_send(());
                }
            }
        });
    }

    /// Re-read both JSON files and update the shared state in place.
    fn reload(&self) {
        Self::reload_into(&self.dir, &self.world_state, &self.health);
    }

    fn reload_into(
        dir: &PathBuf,
        world_state: &Arc<Mutex<Option<MemoryWorldState>>>,
        health: &Arc<Mutex<Option<MemoryHealth>>>,
    ) {
        // world_state_current.json
        let ws_path = dir.join("world_state_current.json");
        if ws_path.exists() {
            match std::fs::read_to_string(&ws_path) {
                Ok(content) => match serde_json::from_str::<MemoryWorldState>(&content) {
                    Ok(ws) => {
                        *world_state.lock().unwrap() = Some(ws);
                    }
                    Err(e) => {
                        eprintln!("memory_state: failed to parse world_state_current.json: {e}");
                    }
                },
                Err(e) => {
                    eprintln!("memory_state: failed to read world_state_current.json: {e}");
                }
            }
        }

        // health.json
        let health_path = dir.join("health.json");
        if health_path.exists() {
            match std::fs::read_to_string(&health_path) {
                Ok(content) => match serde_json::from_str::<MemoryHealth>(&content) {
                    Ok(h) => {
                        *health.lock().unwrap() = Some(h);
                    }
                    Err(e) => {
                        eprintln!("memory_state: failed to parse health.json: {e}");
                    }
                },
                Err(e) => {
                    eprintln!("memory_state: failed to read health.json: {e}");
                }
            }
        }
    }
}
