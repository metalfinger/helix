#!/usr/bin/env python3
"""
Claude Code TUI Status Line
Pure Python, no external dependencies. Pro plan (no cost tracking).

Setup: Add to ~/.claude/settings.json:
  {"statusLine": {"type": "command", "command": "python3 ~/.claude/statusline.py"}}
"""
import json, sys, subprocess, os, time

# Force Unix-style line endings (no \r) — critical on Windows
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(newline="\n")
elif hasattr(sys.stdout, "buffer"):
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, newline="\n")

# Log file for debugging
LOG = os.path.join(os.path.expanduser("~"), ".claude", "statusline-debug.log")

def log_error(section, e):
    try:
        with open(LOG, "a", newline="\n") as f:
            f.write(f"[{time.strftime('%H:%M:%S')}] {section}: {e}\n")
    except Exception:
        pass

# ━━━ Helpers ━━━

def run(cmd, timeout=2):
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return r.stdout.strip()
    except Exception:
        return ""

def fmt_tok(t):
    t = int(t or 0)
    if t >= 1_000_000: return f"{t/1_000_000:.1f}M"
    if t >= 1_000: return f"{t/1_000:.1f}K"
    return str(t)

def fmt_time(ms):
    ms = int(ms or 0)
    if ms <= 0: return "0s"
    s = ms // 1000
    m, s = divmod(s, 60)
    h, m = divmod(m, 60)
    if h > 0: return f"{h}h{m}m{s}s"
    if m > 0: return f"{m}m{s}s"
    return f"{s}s"

def fmt_bytes(b):
    for unit in ["B", "KB", "MB", "GB", "TB"]:
        if b < 1024: return f"{b:.1f} {unit}"
        b /= 1024
    return f"{b:.1f} PB"

def fmt_age(seconds):
    seconds = int(seconds)
    if seconds < 60: return f"{seconds}s ago"
    if seconds < 3600: return f"{seconds // 60}m ago"
    if seconds < 86400: return f"{seconds // 3600}h ago"
    return f"{seconds // 86400}d ago"

# ━━━ Colors ━━━

R = "\033[0m"; B = "\033[1m"; D = "\033[2m"
WHITE = "\033[97m"; GREY = "\033[90m"; CYAN = "\033[96m"; GREEN = "\033[92m"
YELLOW = "\033[93m"; RED = "\033[91m"; MAGENTA = "\033[95m"; BLUE = "\033[94m"
BG_MODEL = "\033[48;2;124;58;237m"
BG_VIM_N = "\033[48;2;30;58;95m"
BG_VIM_I = "\033[48;2;22;78;56m"

# ━━━ Parse input ━━━

try:
    raw = sys.stdin.read()
    data = json.loads(raw)
except Exception as e:
    log_error("parse", e)
    print(f"{GREY}Waiting...{R}")
    sys.exit(0)

# ━━━ Extract fields (with safe defaults) ━━━

def safe_int(val, default=0):
    try:
        if val is None: return default
        return int(float(val))
    except (ValueError, TypeError):
        return default

model_data = data.get("model") or {}
model = model_data.get("display_name") or model_data.get("id") or "Claude"
ctx = data.get("context_window") or {}
cost_data = data.get("cost") or {}
vim_mode = (data.get("vim") or {}).get("mode", "")
workspace = data.get("workspace") or {}

used_pct = safe_int(ctx.get("used_percentage"))
ctx_size = safe_int(ctx.get("context_window_size"))
input_tok = safe_int(ctx.get("total_input_tokens"))
output_tok = safe_int(ctx.get("total_output_tokens"))
cur_usage = ctx.get("current_usage") or {}
cache_read = safe_int(cur_usage.get("cache_read_input_tokens"))
cache_create = safe_int(cur_usage.get("cache_creation_input_tokens"))
duration_ms = safe_int(cost_data.get("total_duration_ms"))
api_ms = safe_int(cost_data.get("total_api_duration_ms"))
lines_add = safe_int(cost_data.get("total_lines_added"))
lines_rm = safe_int(cost_data.get("total_lines_removed"))

# Sanitize path
raw_cwd = str(workspace.get("current_dir") or os.getcwd())
cwd = raw_cwd.replace("\\", "/")
dirname = cwd.rsplit("/", 1)[-1] if "/" in cwd else cwd

# ━━━ Collect output lines ━━━

out = []

# ── Line 1: Model + Dir ──
try:
    L1 = f"{BG_MODEL}{WHITE}{B} {model} {R}  {WHITE}{B}{dirname}{R}"
    if vim_mode:
        bg = BG_VIM_N if vim_mode == "NORMAL" else BG_VIM_I
        L1 += f"  {bg}{WHITE}{B} {vim_mode} {R}"
    out.append(L1)
except Exception as e:
    log_error("model_line", e)
    out.append(f"{BG_MODEL}{WHITE}{B} {model} {R}")

# ── Line 2-3: Git ──
try:
    GIT = ["git", "--no-optional-locks"]
    branch = run(GIT + ["symbolic-ref", "--short", "HEAD"]) or run(GIT + ["rev-parse", "--short", "HEAD"])
    if branch:
        sha = run(GIT + ["rev-parse", "--short", "HEAD"])
        diff_out = run(GIT + ["diff", "--numstat"])
        staged_out = run(GIT + ["diff", "--cached", "--numstat"])
        untracked_out = run(GIT + ["ls-files", "--others", "--exclude-standard"])
        stash_out = run(GIT + ["stash", "list"])

        modified = len(diff_out.splitlines()) if diff_out else 0
        staged = len(staged_out.splitlines()) if staged_out else 0
        untracked = len(untracked_out.splitlines()) if untracked_out else 0
        stash_count = len(stash_out.splitlines()) if stash_out else 0

        ab = run(GIT + ["rev-list", "--left-right", "--count", "HEAD...@{u}"])
        ahead = behind = 0
        if ab and "\t" in ab:
            p = ab.split("\t")
            ahead, behind = int(p[0]), int(p[1])

        parts = [f"{CYAN}{B}{branch}{R}"]
        if sha: parts.append(f"{GREY}@{sha}{R}")
        if staged > 0: parts.append(f"{GREEN}{staged} staged{R}")
        if modified > 0: parts.append(f"{YELLOW}{modified} modified{R}")
        if untracked > 0: parts.append(f"{RED}{untracked} new{R}")
        if stash_count > 0: parts.append(f"{MAGENTA}{stash_count} stashed{R}")
        if ahead > 0: parts.append(f"{GREEN}^ {ahead} ahead{R}")
        if behind > 0: parts.append(f"{RED}v {behind} behind{R}")
        out.append(f"{GREY}Git      {R}{'  '.join(parts)}")

        # Last commit
        last_msg = run(GIT + ["log", "-1", "--format=%s"])
        if last_msg:
            display_msg = (last_msg[:50] + "...") if len(last_msg) > 50 else last_msg
            commit_ts = run(GIT + ["log", "-1", "--format=%ct"])
            cl = f"{GREY}Commit   {R}{D}{display_msg}{R}"
            if commit_ts:
                try:
                    age = int(time.time()) - int(commit_ts)
                    cl += f"  {GREY}({fmt_age(age)}){R}"
                except Exception:
                    pass
            out.append(cl)
except Exception as e:
    log_error("git", e)
    out.append(f"{GREY}Git      {R}{D}error{R}")

# ── Line 4: Context ──
try:
    pct = min(used_pct, 100)
    bar_w = 25
    filled = pct * bar_w // 100
    bar_str = "=" * filled + "-" * (bar_w - filled)
    BARC = RED if pct >= 80 else (YELLOW if pct >= 50 else GREEN)
    total_tok = input_tok + output_tok
    remaining = max(0, ctx_size - total_tok)
    out.append(
        f"{GREY}Context  {R}{BARC}[{bar_str}]{R} {BARC}{B}{pct}%{R}"
        f"  {GREY}{fmt_tok(total_tok)} / {fmt_tok(ctx_size)}{R}"
        f"  {GREY}({fmt_tok(remaining)} remaining){R}"
    )
except Exception as e:
    log_error("context", e)

# ── Line 5: Tokens + Cache ──
try:
    tl = f"{GREY}Tokens   {R}{BLUE}In: {fmt_tok(input_tok)}{R}  {MAGENTA}Out: {fmt_tok(output_tok)}{R}"
    if cache_read > 0 or cache_create > 0:
        total_cache = cache_read + cache_create
        hit = (cache_read / total_cache * 100) if total_cache > 0 else 0
        tl += f"  {GREY}|  Cache{R} {CYAN}Read: {fmt_tok(cache_read)}{R} {YELLOW}Write: {fmt_tok(cache_create)}{R} {GREY}({hit:.0f}% hit){R}"
    out.append(tl)
except Exception as e:
    log_error("tokens", e)

# ── Line 6: Time + Lines ──
try:
    tline = f"{GREY}Time     {R}{CYAN}{fmt_time(duration_ms)}{R} {GREY}total{R}"
    if api_ms:
        wait_ms = max(0, duration_ms - api_ms)
        tline += f"  {BLUE}{fmt_time(api_ms)}{R} {GREY}api{R}"
        if wait_ms > 0:
            tline += f"  {GREY}{fmt_time(wait_ms)} idle{R}"
    lp = []
    if lines_add > 0: lp.append(f"{GREEN}+{lines_add}{R}")
    if lines_rm > 0: lp.append(f"{RED}-{lines_rm}{R}")
    if lp:
        tline += f"  {GREY}|  Lines{R} {' '.join(lp)}"
    out.append(tline)
except Exception as e:
    log_error("time", e)

# Docker + Disk removed — caused timeouts in live sessions

# ━━━ Write HELIX status file ━━━

try:
    import tempfile, pathlib
    status_dir = pathlib.Path.home() / ".ai-status"
    status_dir.mkdir(exist_ok=True)

    # FNV-1a 32-bit hash matching Rust's implementation
    def fnv1a_32(data):
        h = 0x811c9dc5
        for b in data:
            h ^= b
            h = (h * 0x01000193) & 0xFFFFFFFF
        return h

    def cwd_status_filename(cwd_path):
        normalized = cwd_path.replace("\\", "/").lower().rstrip("/")
        # Convert /c/... → c:/... (Git Bash → Windows)
        import re
        normalized = re.sub(r'^/([a-z])/', r'\1:/', normalized)
        h = fnv1a_32(normalized.encode("utf-8"))
        return f"cwd-{h:08x}.json"

    status_path = status_dir / cwd_status_filename(raw_cwd)

    # Read existing (preserves tool/file from PostToolUse hook)
    existing = {}
    if status_path.exists():
        try:
            existing = json.loads(status_path.read_text())
        except Exception:
            pass

    existing["schema_version"] = 1
    existing["cli"] = "claude-code"
    existing["cwd"] = raw_cwd
    existing["model"] = model
    existing["tokens"] = {
        "input": input_tok,
        "output": output_tok,
        "cache_read": cache_read,
        "cache_write": cache_create,
        "context_size": ctx_size,
        "used_pct": used_pct,
    }
    existing["session"] = {
        "start_time": 0,
        "duration_ms": duration_ms,
        "api_duration_ms": api_ms,
    }
    if "activity" not in existing:
        existing["activity"] = {}
    existing["activity"]["lines_added"] = lines_add
    existing["activity"]["lines_removed"] = lines_rm

    # Detect state from token changes (streaming/thinking detection)
    prev_input = existing.get("tokens", {}).get("input", 0)
    prev_state = existing.get("state", "idle")
    prev_ts = existing.get("timestamp", 0)
    now_ts = int(time.time())
    gap = now_ts - prev_ts

    if input_tok > prev_input:
        # Tokens increased — Claude is actively working
        if prev_state in ("coding", "reviewing") and gap <= 4:
            pass  # Keep hook-set state
        elif gap > 0:
            existing["state"] = "streaming"
    # Don't overwrite hook-set states (coding/reviewing) if they're fresh (<5s)
    elif prev_state in ("coding", "reviewing") and gap <= 5:
        pass
    elif prev_state == "streaming" and gap > 3:
        existing["state"] = "idle"

    existing["timestamp"] = now_ts

    tmp = status_path.with_suffix(".tmp")
    tmp.write_text(json.dumps(existing, indent=2))
    tmp.replace(status_path)
except Exception as e:
    log_error("helix_status", e)

# ━━━ Print ━━━

print("\n".join(out))
