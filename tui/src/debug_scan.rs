use sysinfo::{System, ProcessesToUpdate};

pub fn debug_processes() {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_lowercase();
        if name.contains("node") || name.contains("gemini") || name.contains("codex") {
            let cmd: Vec<String> = process.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect();
            let exe = process.exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            let cwd = process.cwd().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            println!("PID:{} NAME:{} EXE:{} CWD:{} CMD:{:?}", pid.as_u32(), name, exe, cwd, cmd);
        }
    }
}
