use sysinfo::{System, ProcessesToUpdate};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DetectedSession {
    pub pid: u32,
    pub cli: String,
    pub cwd: String,
    pub cpu: f32,
    pub memory: u64,
}

pub struct ProcessScanner {
    sys: System,
}

impl ProcessScanner {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        Self { sys }
    }

    pub fn scan(&mut self) -> Vec<DetectedSession> {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);

        let mut sessions = Vec::new();
        let mut seen_pids: HashMap<u32, bool> = HashMap::new();
        let mut seen_clis: HashMap<String, bool> = HashMap::new();

        // Get command lines via platform-specific methods.
        // sysinfo often returns empty cmd()/cwd() on macOS due to sandbox restrictions,
        // and doesn't return cmd on Windows at all. Use external tools as fallback.
        let cmdline_data = get_process_cmdlines();

        for (pid, process) in self.sys.processes() {
            let p = pid.as_u32();
            let name = process.name().to_string_lossy().to_lowercase();
            let exe_path = process.exe()
                .map(|p| p.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            // Try sysinfo cmd() first, fall back to external data
            let sysinfo_cmd = process.cmd().iter()
                .map(|s| s.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            let cmd_str = if !sysinfo_cmd.is_empty() {
                sysinfo_cmd
            } else {
                cmdline_data.get(&p).cloned().unwrap_or_default().to_lowercase()
            };

            // Skip our own process
            if name.contains("helix") || exe_path.contains("helix") {
                continue;
            }

            let cli = if is_claude_code(&name, &exe_path, &cmd_str) {
                "claude-code"
            } else if is_codex(&name, &exe_path, &cmd_str) {
                "codex"
            } else if is_gemini(&name, &exe_path, &cmd_str) {
                "gemini"
            } else if is_aider(&name, &exe_path, &cmd_str) {
                "aider"
            } else {
                continue;
            };

            if seen_pids.contains_key(&p) {
                continue;
            }
            seen_pids.insert(p, true);

            // For node-based CLIs (gemini, aider), deduplicate — multiple node
            // processes belong to one session. For native binaries (claude, codex),
            // each process is a separate session.
            let is_node_cli = name == "node" || name == "node.exe";
            if is_node_cli && seen_clis.contains_key(cli) {
                if let Some(existing) = sessions.iter_mut().find(|s: &&mut DetectedSession| s.cli == cli) {
                    existing.memory += process.memory();
                }
                continue;
            }
            if is_node_cli {
                seen_clis.insert(cli.to_string(), true);
            }

            let cwd = process.cwd()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            sessions.push(DetectedSession {
                pid: p,
                cli: cli.to_string(),
                cwd,
                cpu: process.cpu_usage(),
                memory: process.memory(),
            });
        }

        sessions
    }
}

/// Get command lines for relevant processes using platform-specific methods.
/// On macOS/Linux: uses `ps` which can read cmdlines for same-user processes.
/// On Windows: uses WMI via PowerShell since sysinfo doesn't return cmdlines.
fn get_process_cmdlines() -> HashMap<u32, String> {
    let mut map = HashMap::new();

    #[cfg(not(target_os = "windows"))]
    {
        // ps -eo pid,args returns PID and full command line for all processes
        let output = Command::new("ps")
            .args(["-eo", "pid,args"])
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines().skip(1) {
                let trimmed = line.trim();
                if let Some(space_idx) = trimmed.find(|c: char| c.is_whitespace()) {
                    let pid_str = &trimmed[..space_idx];
                    let cmd = trimmed[space_idx..].trim();
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        // Only store lines relevant to AI CLIs to keep the map small
                        let cmd_lower = cmd.to_lowercase();
                        if cmd_lower.contains("claude")
                            || cmd_lower.contains("codex")
                            || cmd_lower.contains("gemini")
                            || cmd_lower.contains("aider")
                        {
                            map.insert(pid, cmd.to_string());
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell.exe")
            .args([
                "-NoProfile", "-Command",
                "Get-CimInstance Win32_Process -Filter \"Name='node.exe' OR Name='claude.exe' OR Name='codex.exe' OR Name='gemini.exe' OR Name='aider.exe'\" | Select-Object ProcessId, CommandLine, ExecutablePath | ForEach-Object { \"$($_.ProcessId)|$($_.CommandLine)\" }"
            ])
            .output();

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                if let Some((pid_str, cmd)) = line.split_once('|') {
                    if let Ok(pid) = pid_str.trim().parse::<u32>() {
                        map.insert(pid, cmd.to_string());
                    }
                }
            }
        }
    }

    map
}

/// Get command lines for all processes via WMI (PowerShell).
/// Only needed on Windows where sysinfo doesn't return command lines.
#[cfg(target_os = "windows")]
fn get_wmi_cmdlines() -> HashMap<u32, String> {
    let mut map = HashMap::new();

    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile", "-Command",
            "Get-CimInstance Win32_Process -Filter \"Name='node.exe' OR Name='claude.exe' OR Name='codex.exe' OR Name='gemini.exe' OR Name='aider.exe'\" | Select-Object ProcessId, CommandLine, ExecutablePath | ForEach-Object { \"$($_.ProcessId)|$($_.CommandLine)\" }"
        ])
        .output();

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            if let Some((pid_str, cmd)) = line.split_once('|') {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    map.insert(pid, cmd.to_string());
                }
            }
        }
    }

    map
}

fn is_claude_code(name: &str, exe_path: &str, cmd_str: &str) -> bool {
    if !name.contains("claude") && !cmd_str.contains("claude") {
        return false;
    }
    // Filter out Windows UWP/Store installs
    if exe_path.contains("windowsapps") || exe_path.contains("claude_1.") {
        return false;
    }
    if name.contains("helix") || exe_path.contains("helix") {
        return false;
    }
    if exe_path.contains(".local/bin/claude") || exe_path.contains(".local\\bin\\claude") {
        return true;
    }
    if cmd_str.contains("@anthropic-ai/claude-code") || cmd_str.contains("claude-code") {
        return true;
    }
    // Match bare "claude" binary (native install)
    let bare_name = name.trim_end_matches(".exe");
    if bare_name == "claude" && !exe_path.contains("windowsapps") {
        return true;
    }
    false
}

fn is_codex(name: &str, exe_path: &str, cmd_str: &str) -> bool {
    let bare_name = name.trim_end_matches(".exe");
    if bare_name.contains("codex") || exe_path.contains("codex") {
        return true;
    }
    if cmd_str.contains("@openai/codex") || cmd_str.contains("openai\\codex") {
        return true;
    }
    false
}

fn is_gemini(name: &str, _exe_path: &str, cmd_str: &str) -> bool {
    let bare_name = name.trim_end_matches(".exe");
    if bare_name.contains("gemini") {
        return true;
    }
    if cmd_str.contains("gemini-cli") || cmd_str.contains("gemini_cli") {
        return true;
    }
    if cmd_str.contains("@google/gemini") || cmd_str.contains("@google\\gemini") {
        return true;
    }
    false
}

fn is_aider(name: &str, _exe_path: &str, cmd_str: &str) -> bool {
    let bare_name = name.trim_end_matches(".exe");
    bare_name == "aider" || cmd_str.contains("aider")
}
