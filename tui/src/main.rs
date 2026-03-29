use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod ambient;
mod app;
mod config;
mod debug_scan;
mod history;
mod layout;
mod mascot;
mod memory_state;
mod scanner;
mod sessions;
mod status;
mod theme;
mod widgets;

#[derive(Parser)]
#[command(name = "helix", about = "HELIX — Animated TUI dashboard for AI CLIs")]
struct Cli {
    /// Mode: dashboard, statusline, minimal
    #[arg(long, default_value = "dashboard")]
    mode: String,

    /// Path to config file
    #[arg(long)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Debug: dump all detected node/gemini/codex processes
    DebugScan,

    /// Update the status file (used by CLI hooks, fire-and-forget)
    StatusUpdate {
        /// CLI name (claude-code, codex, etc.)
        #[arg(long, default_value = "claude-code")]
        cli: String,

        /// State (thinking, coding, idle, error, done, etc.)
        #[arg(long, default_value = "idle")]
        state: String,

        /// Tool that was just used
        #[arg(long, default_value = "")]
        tool: String,

        /// File that was just operated on
        #[arg(long, default_value = "")]
        file: String,

        /// Human-readable description of the tool action
        #[arg(long, default_value = "")]
        description: String,

        /// Detail context (line counts, command preview, pattern)
        #[arg(long, default_value = "")]
        detail: String,

        /// Working directory of the CLI session
        #[arg(long, default_value = "")]
        cwd: String,

        /// Current git branch
        #[arg(long, default_value = "")]
        git_branch: String,

        /// Tool result summary
        #[arg(long, default_value = "")]
        result: String,

        /// Tool success flag (1 = success, 0 = failure)
        #[arg(long, default_value = "1")]
        success: String,

        /// Instance ID (unused, accepted for forward-compat)
        #[arg(long, default_value = "0")]
        instance_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Handle subcommands
    if let Some(Commands::DebugScan) = &cli.command {
        debug_scan::debug_processes();
        return Ok(());
    }

    if let Some(Commands::StatusUpdate { cli: cli_name, state, tool, file, description, detail, result, success, cwd, git_branch, instance_id: _ }) = cli.command {
        let home = dirs::home_dir().unwrap_or_default();
        let status_dir = home.join(".ai-status");
        let _ = std::fs::create_dir_all(&status_dir);

        // Use cwd-based filename so hook + statusline write to the SAME file
        let status_path = if !cwd.is_empty() {
            status_dir.join(status::status_filename(&cwd))
        } else {
            // Fallback if no cwd provided
            status_dir.join("unknown.json")
        };

        let mut status: serde_json::Value = if status_path.exists() {
            let content = std::fs::read_to_string(&status_path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        status["schema_version"] = serde_json::json!(1);
        status["cli"] = serde_json::json!(cli_name);
        status["state"] = serde_json::json!(state);
        status["timestamp"] = serde_json::json!(chrono::Utc::now().timestamp());

        if !cwd.is_empty() {
            status["cwd"] = serde_json::json!(cwd);
        }

        if !tool.is_empty() {
            if status.get("activity").is_none() {
                status["activity"] = serde_json::json!({});
            }
            status["activity"]["last_tool"] = serde_json::json!(tool);
            // Always clear description/detail when a new tool fires
            // (prevents stale data from previous tool leaking through)
            status["activity"]["last_description"] = serde_json::json!(description);
            status["activity"]["last_detail"] = serde_json::json!(detail);
            status["activity"]["last_file"] = serde_json::json!(file);
            status["activity"]["last_result"] = serde_json::json!(result);
            status["activity"]["last_success"] = serde_json::json!(success == "1");
        }

        if !git_branch.is_empty() {
            if status.get("git").is_none() {
                status["git"] = serde_json::json!({});
            }
            status["git"]["branch"] = serde_json::json!(git_branch);
        }

        // Write atomically
        let tmp_path = status_path.with_extension("tmp");
        if let Ok(content) = serde_json::to_string_pretty(&status) {
            let _ = std::fs::write(&tmp_path, &content);
            let _ = std::fs::rename(&tmp_path, &status_path);
        }

        return Ok(());
    }

    let config = match &cli.config {
        Some(path) => config::Config::load(path)?,
        None => {
            let default_path = dirs::home_dir()
                .unwrap_or_default()
                .join(".helix-tui")
                .join("config.toml");
            if default_path.exists() {
                config::Config::load(&default_path)?
            } else {
                config::Config::default()
            }
        }
    };

    match cli.mode.as_str() {
        "dashboard" => {
            let mut app = app::App::new(config);
            app.run().await?;
        }
        "statusline" => {
            // Read Claude Code's statusline JSON from stdin, write to status file,
            // and output a formatted statusline
            let mut input = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut input).ok();

            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&input) {
                let home = dirs::home_dir().unwrap_or_default();
                let status_dir = home.join(".ai-status");
                let _ = std::fs::create_dir_all(&status_dir);

                // Get cwd from the input data for filename
                let workspace = data.get("workspace").and_then(|w| w.as_object());
                let cwd_str = workspace
                    .and_then(|w| w.get("current_dir"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let status_path = if !cwd_str.is_empty() {
                    status_dir.join(status::status_filename(cwd_str))
                } else {
                    status_dir.join("unknown.json")
                };

                // Read existing status (preserves tool/file from PostToolUse hook)
                let mut status: serde_json::Value = if status_path.exists() {
                    let content = std::fs::read_to_string(&status_path).unwrap_or_default();
                    serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };

                // Extract Claude Code's native fields
                let model_data = data.get("model").and_then(|m| m.as_object());
                let ctx = data.get("context_window").and_then(|c| c.as_object());
                let cost = data.get("cost").and_then(|c| c.as_object());
                let cur_usage = ctx.and_then(|c| c.get("current_usage")).and_then(|u| u.as_object());

                status["schema_version"] = serde_json::json!(1);
                status["cli"] = serde_json::json!("claude-code");

                if !cwd_str.is_empty() {
                    status["cwd"] = serde_json::json!(cwd_str);
                }

                if let Some(m) = model_data {
                    status["model"] = serde_json::json!(
                        m.get("display_name").and_then(|v| v.as_str())
                         .or_else(|| m.get("id").and_then(|v| v.as_str()))
                         .unwrap_or("Claude")
                    );
                }

                let prev_input_tokens = status.get("tokens")
                    .and_then(|t| t.get("input"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let prev_state = status.get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("idle")
                    .to_string();
                let prev_timestamp = status.get("timestamp")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                if let Some(c) = ctx {
                    let input_tok = c.get("total_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let output_tok = c.get("total_output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    let ctx_size = c.get("context_window_size").and_then(|v| v.as_u64()).unwrap_or(0);
                    let used_pct = c.get("used_percentage").and_then(|v| v.as_u64()).unwrap_or(0);

                    status["tokens"] = serde_json::json!({
                        "input": input_tok,
                        "output": output_tok,
                        "cache_read": cur_usage.and_then(|u| u.get("cache_read_input_tokens")).and_then(|v| v.as_u64()).unwrap_or(0),
                        "cache_write": cur_usage.and_then(|u| u.get("cache_creation_input_tokens")).and_then(|v| v.as_u64()).unwrap_or(0),
                        "context_size": ctx_size,
                        "used_pct": used_pct
                    });
                }

                if let Some(c) = cost {
                    status["session"] = serde_json::json!({
                        "start_time": 0,
                        "duration_ms": c.get("total_duration_ms").and_then(|v| v.as_u64()).unwrap_or(0),
                        "api_duration_ms": c.get("total_api_duration_ms").and_then(|v| v.as_u64()).unwrap_or(0)
                    });

                    if let Some(added) = c.get("total_lines_added").and_then(|v| v.as_u64()) {
                        if status.get("activity").is_none() {
                            status["activity"] = serde_json::json!({});
                        }
                        status["activity"]["lines_added"] = serde_json::json!(added);
                    }
                    if let Some(removed) = c.get("total_lines_removed").and_then(|v| v.as_u64()) {
                        if status.get("activity").is_none() {
                            status["activity"] = serde_json::json!({});
                        }
                        status["activity"]["lines_removed"] = serde_json::json!(removed);
                    }
                }

                {
                    let now_ts = chrono::Utc::now().timestamp();
                    let new_input = ctx.and_then(|c| c.get("total_input_tokens")).and_then(|v| v.as_u64()).unwrap_or(0);
                    let gap = now_ts - prev_timestamp;
                    if new_input > prev_input_tokens && (
                        (gap > 2 && prev_state != "coding" && prev_state != "reviewing")
                        || ((prev_state == "coding" || prev_state == "reviewing") && gap > 4)
                    ) {
                        status["state"] = serde_json::json!("streaming");
                    }
                }

                status["timestamp"] = serde_json::json!(chrono::Utc::now().timestamp());

                // Write status file
                let tmp_path = status_path.with_extension("tmp");
                if let Ok(content) = serde_json::to_string_pretty(&status) {
                    let _ = std::fs::write(&tmp_path, &content);
                    let _ = std::fs::rename(&tmp_path, &status_path);
                }

                // Also output a simple statusline for Claude Code to display
                let used_pct = ctx.and_then(|c| c.get("used_percentage")).and_then(|v| v.as_u64()).unwrap_or(0);
                let model_name = status.get("model").and_then(|v| v.as_str()).unwrap_or("Claude");
                let state = status.get("state").and_then(|v| v.as_str()).unwrap_or("idle");
                print!("\x1b[96mHELIX\x1b[0m \x1b[92m{}\x1b[0m {}% \x1b[90m{}\x1b[0m", model_name, used_pct, state);
            }
        }
        "minimal" => {
            eprintln!("Minimal mode not yet implemented");
        }
        _ => {
            eprintln!("Unknown mode: {}", cli.mode);
        }
    }

    Ok(())
}
