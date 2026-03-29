use std::path::{Path, PathBuf};
use std::collections::HashMap;
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct SessionData {
    pub session_id: String,
    pub project_path: String,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub message_count: u32,
    pub tool_calls: Vec<ToolCall>,
    pub last_tool: String,
    pub last_file: String,
    pub model: String,
    pub lines_added: u64,
    pub lines_removed: u64,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub tool: String,
    pub file: String,
    pub timestamp: String,
}

/// Reads Claude Code's JSONL session files to extract real token usage and tool history
pub fn read_claude_sessions(claude_dir: &Path) -> Vec<SessionData> {
    let projects_dir = claude_dir.join("projects");
    if !projects_dir.exists() {
        return Vec::new();
    }

    let mut sessions = Vec::new();

    // Scan all project directories
    if let Ok(project_entries) = std::fs::read_dir(&projects_dir) {
        for project_entry in project_entries.flatten() {
            let project_path = project_entry.path();
            if !project_path.is_dir() {
                continue;
            }

            // Find .jsonl session files in each project dir
            if let Ok(files) = std::fs::read_dir(&project_path) {
                for file in files.flatten() {
                    let path = file.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        if let Some(session) = parse_session_file(&path, &project_path) {
                            sessions.push(session);
                        }
                    }
                }
            }
        }
    }

    // Sort by most recent (highest token count = most active)
    sessions.sort_by(|a, b| {
        let a_total = a.total_input_tokens + a.total_output_tokens;
        let b_total = b.total_input_tokens + b.total_output_tokens;
        b_total.cmp(&a_total)
    });

    sessions
}

/// Find the most recently modified session file (likely the active one)
pub fn find_active_session(claude_dir: &Path) -> Option<SessionData> {
    let projects_dir = claude_dir.join("projects");
    if !projects_dir.exists() {
        return None;
    }

    let mut newest: Option<(PathBuf, PathBuf, std::time::SystemTime)> = None;

    if let Ok(project_entries) = std::fs::read_dir(&projects_dir) {
        for project_entry in project_entries.flatten() {
            let project_path = project_entry.path();
            if !project_path.is_dir() {
                continue;
            }

            if let Ok(files) = std::fs::read_dir(&project_path) {
                for file in files.flatten() {
                    let path = file.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        if let Ok(meta) = path.metadata() {
                            if let Ok(modified) = meta.modified() {
                                if newest.as_ref().map_or(true, |(_, _, t)| modified > *t) {
                                    newest = Some((path, project_path.clone(), modified));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    newest.and_then(|(path, project_path, _)| parse_session_file(&path, &project_path))
}

fn parse_session_file(path: &Path, project_path: &Path) -> Option<SessionData> {
    let content = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return None;
    }

    let session_id = path.file_stem()?.to_string_lossy().to_string();
    let project_name = project_path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut data = SessionData {
        session_id,
        project_path: project_name,
        ..Default::default()
    };

    let mut recent_tools: Vec<ToolCall> = Vec::new();

    // Parse JSONL — read last N lines for efficiency (recent activity)
    let start = if lines.len() > 500 { lines.len() - 500 } else { 0 };

    for line in &lines[start..] {
        if let Ok(entry) = serde_json::from_str::<Value>(line) {
            // Extract token usage from message entries
            if let Some(usage) = entry.get("message").and_then(|m| m.get("usage")) {
                data.total_input_tokens += usage.get("input_tokens")
                    .and_then(|v| v.as_u64()).unwrap_or(0);
                data.total_output_tokens += usage.get("output_tokens")
                    .and_then(|v| v.as_u64()).unwrap_or(0);

                if let Some(cache) = usage.get("cache_read_input_tokens") {
                    data.cache_read_tokens += cache.as_u64().unwrap_or(0);
                }
                if let Some(cache) = usage.get("cache_creation_input_tokens") {
                    data.cache_write_tokens += cache.as_u64().unwrap_or(0);
                }

                // Get model from message
                if let Some(model) = entry.get("message").and_then(|m| m.get("model")).and_then(|v| v.as_str()) {
                    data.model = model.to_string();
                }

                data.message_count += 1;
            }

            // Extract tool use
            if let Some(content) = entry.get("message").and_then(|m| m.get("content")) {
                if let Some(arr) = content.as_array() {
                    for block in arr {
                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                            let tool = block.get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();

                            let file = block.get("input")
                                .and_then(|i| {
                                    i.get("file_path").or(i.get("command")).or(i.get("pattern"))
                                })
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();

                            // Truncate file path (char-safe)
                            let file_short = if file.chars().count() > 60 {
                                let skip = file.chars().count() - 57;
                                format!("...{}", file.chars().skip(skip).collect::<String>())
                            } else {
                                file.clone()
                            };

                            recent_tools.push(ToolCall {
                                tool: tool.clone(),
                                file: file_short.clone(),
                                timestamp: String::new(),
                            });

                            data.last_tool = tool;
                            data.last_file = file_short;
                        }
                    }
                }
            }
        }
    }

    // Keep last 30 tool calls
    if recent_tools.len() > 30 {
        let start = recent_tools.len() - 30;
        recent_tools = recent_tools[start..].to_vec();
    }
    data.tool_calls = recent_tools;

    // Only return sessions with actual data
    if data.message_count > 0 || data.total_input_tokens > 0 {
        Some(data)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool: String,
    pub file: String,
    pub result_summary: String,
    pub success: bool,
}

/// Extract recent tool results from the active session's JSONL file.
pub fn read_recent_tool_results(claude_dir: &Path) -> Vec<ToolResult> {
    let projects_dir = claude_dir.join("projects");
    if !projects_dir.exists() {
        return Vec::new();
    }

    // Find most recently modified JSONL
    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
    if let Ok(project_entries) = std::fs::read_dir(&projects_dir) {
        for project_entry in project_entries.flatten() {
            let project_path = project_entry.path();
            if !project_path.is_dir() { continue; }
            if let Ok(files) = std::fs::read_dir(&project_path) {
                for file in files.flatten() {
                    let path = file.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        if let Ok(meta) = path.metadata() {
                            if let Ok(modified) = meta.modified() {
                                if newest.as_ref().map_or(true, |(_, t)| modified > *t) {
                                    newest = Some((path, modified));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let path = match newest {
        Some((p, _)) => p,
        None => return Vec::new(),
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let lines: Vec<&str> = content.lines().collect();
    let start = if lines.len() > 200 { lines.len() - 200 } else { 0 };

    let mut results = Vec::new();
    let mut last_tool_name = String::new();
    let mut last_tool_file = String::new();

    for line in &lines[start..] {
        if let Ok(entry) = serde_json::from_str::<Value>(line) {
            // IMPORTANT: Claude Code JSONL has TWO formats:
            // 1. Top-level: {"type":"tool_result","content":[{"type":"text","text":"..."}]}
            // 2. Inside message: {"message":{"content":[{"type":"tool_use",...}]}}

            // Check for top-level tool_result entries
            if entry.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                let text = entry.get("content")
                    .and_then(|c| {
                        if let Some(arr) = c.as_array() {
                            arr.first().and_then(|b| b.get("text")).and_then(|t| t.as_str())
                        } else {
                            c.as_str()
                        }
                    })
                    .unwrap_or("");

                let (summary, success) = summarize_result(&last_tool_name, text);
                if !summary.is_empty() {
                    results.push(ToolResult {
                        tool: last_tool_name.clone(),
                        file: last_tool_file.clone(),
                        result_summary: summary,
                        success,
                    });
                }
                continue;
            }

            // Track tool_use blocks inside message.content
            if let Some(content) = entry.get("message").and_then(|m| m.get("content")) {
                if let Some(arr) = content.as_array() {
                    for block in arr {
                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                            last_tool_name = block.get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            last_tool_file = block.get("input")
                                .and_then(|i| i.get("file_path").or(i.get("command")).or(i.get("pattern")))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                        }
                    }
                }
            }
        }
    }

    results
}

fn summarize_result(tool: &str, text: &str) -> (String, bool) {
    let text_lower = text.to_lowercase();
    match tool {
        "Bash" => {
            let has_error = text_lower.contains("error") || text_lower.contains("failed")
                || text_lower.contains("not found") || text_lower.contains("permission denied");
            let meaningful: Vec<&str> = text.lines()
                .filter(|l| !l.trim().is_empty())
                .take(2)
                .collect();
            let summary = meaningful.join(" | ");
            let truncated = if summary.len() > 80 { format!("{}…", &summary[..79]) } else { summary };
            (truncated, !has_error)
        }
        "Edit" => {
            if text_lower.contains("not found") || text_lower.contains("not unique") {
                (text.lines().next().unwrap_or("error").to_string(), false)
            } else {
                ("applied".to_string(), true)
            }
        }
        "Write" => ("written".to_string(), true),
        "Read" => {
            let line_count = text.lines().count();
            (format!("{} lines", line_count), true)
        }
        "Grep" => {
            if text.trim().is_empty() || text_lower.contains("no files found") || text_lower.contains("no matches") {
                ("no matches".to_string(), true)
            } else {
                let count = text.lines().count();
                (format!("{} matches", count), true)
            }
        }
        "Glob" => {
            if text.trim().is_empty() || text_lower.contains("no files") {
                ("no files".to_string(), true)
            } else {
                let count = text.lines().filter(|l| !l.trim().is_empty()).count();
                (format!("{} files", count), true)
            }
        }
        _ => {
            let first_line = text.lines().next().unwrap_or("");
            let truncated = if first_line.len() > 60 { format!("{}…", &first_line[..59]) } else { first_line.to_string() };
            (truncated, true)
        }
    }
}
