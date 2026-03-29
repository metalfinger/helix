use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActivitySnapshot {
    pub time: String,
    pub tool: String,
    pub file: String,
    pub description: String,
    pub status: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryEntry {
    pub id: String,
    pub cwd: String,
    pub cli: String,
    pub model: String,
    pub git_branch: String,
    pub started_at: String,
    pub ended_at: String,
    pub duration_ms: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub context_used_pct: u32,
    pub last_state: String,
    pub last_tool: String,
    pub last_file: String,
    pub last_description: String,
    pub activities: Vec<ActivitySnapshot>,
    pub files_touched: Vec<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Look up the most recent Claude session ID for a given project directory.
/// Reads ~/.claude/history.jsonl which has lines like:
/// {"sessionId":"uuid","project":"C:\\path\\to\\project","timestamp":123456789}
pub fn find_claude_session_id(cwd: &str) -> Option<String> {
    let path = dirs::home_dir()?.join(".claude").join("history.jsonl");
    let file = std::fs::File::open(&path).ok()?;
    let reader = std::io::BufReader::new(file);

    // Normalize cwd for comparison (lowercase, forward slashes)
    let cwd_norm = cwd.to_lowercase().replace('\\', "/");
    let cwd_norm = cwd_norm.trim_end_matches('/');

    let mut best: Option<(u64, String)> = None; // (timestamp, sessionId)

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        // Parse as generic JSON value to extract sessionId + project + timestamp
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let project = match v.get("project").and_then(|p| p.as_str()) {
            Some(p) => p.to_lowercase().replace('\\', "/"),
            None => continue,
        };
        let project = project.trim_end_matches('/');
        if project != cwd_norm {
            continue;
        }
        let session_id = match v.get("sessionId").and_then(|s| s.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let ts = v.get("timestamp").and_then(|t| t.as_u64()).unwrap_or(0);
        if best.as_ref().map_or(true, |(best_ts, _)| ts > *best_ts) {
            best = Some((ts, session_id));
        }
    }

    best.map(|(_, id)| id)
}

pub fn history_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    home.join(".ai-status").join("history.jsonl")
}

pub fn read_history() -> Vec<HistoryEntry> {
    let path = history_path();
    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let reader = std::io::BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<HistoryEntry>(&line) {
            entries.push(entry);
        }
    }
    entries
}

pub fn append_history(entry: &HistoryEntry) -> std::io::Result<()> {
    let path = history_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    let json = serde_json::to_string(entry)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    writeln!(file, "{}", json)?;
    Ok(())
}

pub fn prune_history(entries: &mut Vec<HistoryEntry>) {
    let now = Utc::now().timestamp() as u64;
    let thirty_days_ms = 30 * 24 * 60 * 60 * 1000_u64;
    let original_len = entries.len();

    entries.retain(|entry| {
        // Parse ended_at as RFC3339 timestamp (prune based on when session ended)
        if let Ok(dt) = DateTime::parse_from_rfc3339(&entry.ended_at) {
            let entry_ms = dt.timestamp_millis() as u64;
            let now_ms = now * 1000;
            now_ms.saturating_sub(entry_ms) < thirty_days_ms
        } else {
            // Keep entries with unparseable timestamps
            true
        }
    });

    if entries.len() < original_len {
        let _ = rewrite_history(entries);
    }
}

pub fn rewrite_history(entries: &[HistoryEntry]) -> std::io::Result<()> {
    let path = history_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("tmp");
    {
        let mut file = std::fs::File::create(&tmp_path)?;
        for entry in entries {
            let json = serde_json::to_string(entry)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            writeln!(file, "{}", json)?;
        }
    }
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}

pub fn format_duration_short(ms: u64) -> String {
    let total_secs = ms / 1000;
    if total_secs < 60 {
        format!("{}s", total_secs)
    } else if total_secs < 3600 {
        format!("{}m", total_secs / 60)
    } else {
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        format!("{}h{}m", hours, mins)
    }
}

pub fn write_resume_context(entry: &HistoryEntry) -> std::io::Result<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let path = home.join(".ai-status").join("resume-context.md");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let project = entry.cwd.rsplit(&['/', '\\'][..]).next().unwrap_or(&entry.cwd);
    let duration = format_duration_short(entry.duration_ms);
    let ended_ts = DateTime::parse_from_rfc3339(&entry.ended_at)
        .map(|dt| dt.timestamp() as u64)
        .unwrap_or(0);
    let relative = format_relative_time(ended_ts);

    let mut md = String::new();
    md.push_str("# Resume Context\n\n");
    md.push_str("## Session\n");
    md.push_str(&format!("- **Project:** {}\n", project));
    md.push_str(&format!("- **Branch:** {}\n", entry.git_branch));
    md.push_str(&format!("- **Duration:** {}\n", duration));
    md.push_str(&format!("- **Context used:** {}%\n", entry.context_used_pct));
    md.push_str(&format!("- **Ended:** {}\n\n", relative));

    md.push_str("## Last Activity\n");
    md.push_str(&format!("- {} {} — \"{}\"\n\n", entry.last_tool, entry.last_file, entry.last_description));

    if !entry.files_touched.is_empty() {
        md.push_str("## Files Touched\n");
        for f in &entry.files_touched {
            md.push_str(&format!("- {}\n", f));
        }
        md.push('\n');
    }

    if !entry.activities.is_empty() {
        md.push_str("## Recent Activity Log\n");
        for a in &entry.activities {
            md.push_str(&format!("- {} {} {} — {}\n", a.time, a.tool, a.file, a.description));
        }
        md.push('\n');
    }

    std::fs::write(&path, &md)?;
    Ok(path)
}

/// Spawn a new Claude Code session to resume a past session.
///
/// Uses `claude --resume <session_id>` if we have the session ID,
/// otherwise falls back to `claude -c` (continue most recent in cwd).
pub fn spawn_resume(entry: &HistoryEntry) -> std::io::Result<()> {
    let cwd = &entry.cwd;

    // Build claude args: prefer --resume <id> for exact session, fall back to -c
    let claude_args: Vec<String> = if let Some(ref sid) = entry.session_id {
        vec!["-r".to_string(), sid.clone()]
    } else {
        vec!["-c".to_string()]
    };

    #[cfg(target_os = "windows")]
    {
        // Windows Terminal: split-pane in the current window
        let mut wt_args = vec![
            "-w".to_string(), "0".to_string(),
            "split-pane".to_string(),
            "-d".to_string(), cwd.to_string(),
            "claude".to_string(),
        ];
        wt_args.extend(claude_args.clone());

        let wt_result = std::process::Command::new("wt")
            .args(&wt_args)
            .spawn();

        if wt_result.is_ok() {
            return Ok(());
        }

        // Fallback: new cmd window
        let claude_flag = claude_args.join(" ");
        std::process::Command::new("cmd")
            .args(["/c", "start", "cmd", "/k",
                &format!("cd /d \"{}\" && claude {}", cwd, claude_flag)])
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        let claude_flag = claude_args.join(" ");
        // Try WezTerm first (if installed), then fall back to macOS Terminal.app
        let wez_result = std::process::Command::new("wezterm")
            .args(["cli", "split-pane", "--cwd", cwd, "--", "claude"])
            .args(&claude_args)
            .spawn();

        if wez_result.is_ok() {
            return Ok(());
        }

        // Fallback: open a new Terminal.app window via osascript
        let script = format!(
            "tell application \"Terminal\"\n  do script \"cd '{}' && claude {}\"\n  activate\nend tell",
            cwd.replace('\'', "'\\''"), claude_flag
        );
        std::process::Command::new("osascript")
            .args(["-e", &script])
            .spawn()?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // Linux: try common terminal emulators
        let claude_flag = claude_args.join(" ");
        let shell_cmd = format!("cd '{}' && claude {}", cwd.replace('\'', "'\\''"), claude_flag);
        // Try xdg-terminal-exec (freedesktop standard), then common terminals
        let terminals = [
            ("xdg-terminal-exec", vec!["sh", "-c", &shell_cmd]),
        ];
        for (term, args) in &terminals {
            if std::process::Command::new(term).args(args).spawn().is_ok() {
                return Ok(());
            }
        }
        // Last resort: just spawn claude directly
        std::process::Command::new("claude")
            .args(&claude_args)
            .current_dir(cwd)
            .spawn()?;
    }

    Ok(())
}

pub fn format_relative_time(timestamp: u64) -> String {
    let now = Utc::now().timestamp() as u64;
    let diff = now.saturating_sub(timestamp);

    if diff < 60 {
        return "just now".to_string();
    }
    if diff < 3600 {
        return format!("{}m ago", diff / 60);
    }
    if diff < 86400 {
        return format!("{}h ago", diff / 3600);
    }
    if diff < 172800 {
        return "yesterday".to_string();
    }

    // Format as "Mon DD"
    if let Some(dt) = DateTime::from_timestamp(timestamp as i64, 0) {
        dt.format("%b %d").to_string()
    } else {
        "unknown".to_string()
    }
}
