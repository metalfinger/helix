#!/usr/bin/env bash
# HELIX PostToolUse hook — extracts rich data from tool_input, updates status
# Find helix binary — check PATH first, then common install locations
HELIX_BIN=$(command -v helix 2>/dev/null || echo "")
[[ -z "$HELIX_BIN" && -f "$HOME/bin/helix.exe" ]] && HELIX_BIN="$HOME/bin/helix.exe"
[[ -z "$HELIX_BIN" && -f "$HOME/bin/helix" ]] && HELIX_BIN="$HOME/bin/helix"
[[ -z "$HELIX_BIN" ]] && exit 0

# Read stdin (Claude Code sends tool info as JSON)
input=$(cat)

# Extract fields using python3 (reliable JSON parsing, fail gracefully)
eval "$(python3 -c "
import json, sys, os
try:
    d = json.loads(sys.stdin.read())
    tool = d.get('tool_name', '')
    inp = d.get('tool_input', {})
    resp = d.get('tool_response', {})
    if isinstance(inp, str):
        inp = json.loads(inp) if inp else {}
    if isinstance(resp, str):
        resp_str = resp
        resp = {}
    else:
        resp_str = ''

    file_path = inp.get('file_path', inp.get('command', inp.get('pattern', '')))
    if file_path is None:
        file_path = ''
    file_path = str(file_path)[:120]

    desc = ''
    detail = ''
    result = ''
    success = True

    if tool == 'Read':
        desc = 'Reading'
        detail = ''
        # Response: {type, file: {filePath, content, numLines, totalLines}}
        f = resp.get('file', {}) if isinstance(resp, dict) else {}
        total = f.get('totalLines', 0) if isinstance(f, dict) else 0
        num = f.get('numLines', 0) if isinstance(f, dict) else 0
        if total > 0:
            result = '%d lines (of %d)' % (num, total) if num < total else '%d lines' % total
        elif num > 0:
            result = '%d lines' % num
    elif tool == 'Edit':
        desc = 'Editing'
        old_s = inp.get('old_string', '')
        new_s = inp.get('new_string', '')
        old_n = old_s.count(chr(10)) + (1 if old_s else 0)
        new_n = new_s.count(chr(10)) + (1 if new_s else 0)
        detail = '+%d -%d' % (new_n, old_n)
        # Check response for success/failure
        r_text = resp_str or str(resp)
        if 'not found' in r_text.lower() or 'not unique' in r_text.lower():
            success = False
            result = r_text.split(chr(10))[0][:60]
        else:
            result = 'applied'
    elif tool == 'Write':
        desc = 'Creating'
        content = inp.get('content', '')
        lines_n = content.count(chr(10)) + (1 if content else 0)
        detail = '%d lines' % lines_n
        result = 'written'
    elif tool == 'Bash':
        desc = inp.get('description', '')[:60] or 'Running'
        cmd = inp.get('command', '')
        # Strip cd prefix — it's noise, show the actual command
        if ' && ' in cmd:
            parts = cmd.split(' && ')
            meaningful = [p for p in parts if not p.strip().startswith('cd ')]
            cmd = ' && '.join(meaningful) if meaningful else parts[-1]
        if cmd.startswith(\"bash -c '\") or cmd.startswith('bash -c \"'):
            cmd = cmd[9:]
        for suffix in [' 2>&1', ' 2>/dev/null', ' &>/dev/null', ' >/dev/null']:
            cmd = cmd.replace(suffix, '')
        detail = cmd.strip()[:80]
        # Response: {stdout, stderr, interrupted, isImage, noOutputExpected}
        r_stdout = resp.get('stdout', '') if isinstance(resp, dict) else ''
        r_stderr = resp.get('stderr', '') if isinstance(resp, dict) else ''
        interrupted = resp.get('interrupted', False) if isinstance(resp, dict) else False
        if interrupted:
            result = 'interrupted'
            success = False
        elif r_stdout and r_stdout.strip():
            lines = [l for l in r_stdout.strip().split(chr(10)) if l.strip()][:2]
            result = ' | '.join(lines)[:80]
        elif r_stderr and r_stderr.strip():
            first_err = r_stderr.strip().split(chr(10))[0][:60]
            result = first_err
        if r_stderr and ('error' in r_stderr.lower() or 'failed' in r_stderr.lower()):
            success = False
        if r_stdout and ('error' in r_stdout.lower() or 'failed' in r_stdout.lower() or 'not found' in r_stdout.lower()):
            success = False
    elif tool == 'Grep':
        desc = 'Searching'
        pat = inp.get('pattern', '')
        path = inp.get('path', '.')
        pat_display = (str(pat)[:27] + '...') if len(str(pat)) > 30 else str(pat)
        detail = '\"' + pat_display + '\" in ' + os.path.basename(str(path))
        # Response: {mode, numFiles, filenames, content, numLines}
        n_lines = resp.get('numLines', 0) if isinstance(resp, dict) else 0
        n_files = resp.get('numFiles', 0) if isinstance(resp, dict) else 0
        mode = resp.get('mode', '') if isinstance(resp, dict) else ''
        if mode == 'files_with_matches':
            result = '%d files' % n_files if n_files > 0 else 'no matches'
        elif mode == 'count':
            result = '%d matches' % n_lines if n_lines > 0 else 'no matches'
        else:
            result = '%d matches' % n_lines if n_lines > 0 else 'no matches'
    elif tool == 'Glob':
        desc = 'Finding'
        detail = str(inp.get('pattern', ''))[:40]
        # Response: {filenames, durationMs, numFiles, truncated}
        n = resp.get('numFiles', 0) if isinstance(resp, dict) else 0
        result = '%d files' % n if n > 0 else 'no files'
    elif tool == 'Agent':
        desc = 'Spawning agent'
        prompt = str(inp.get('prompt', inp.get('description', '')))
        detail = (prompt[:37] + '...') if len(prompt) > 40 else prompt
        r_text = resp_str or str(resp.get('content', resp.get('result', '')))
        if r_text and len(r_text) > 5 and r_text != '{}' and r_text != 'None':
            result = (r_text[:57] + '...') if len(r_text) > 60 else r_text
            result = result.split(chr(10))[0][:60]
    elif tool == 'Skill':
        desc = 'Using skill'
        detail = str(inp.get('skill', ''))[:40]
    elif tool == 'WebSearch':
        desc = 'Web search'
        detail = str(inp.get('query', ''))[:60]
    elif tool == 'WebFetch':
        desc = 'Fetching'
        detail = str(inp.get('url', ''))[:60]
    elif tool == 'TaskCreate':
        desc = 'Task created'
        detail = str(inp.get('subject', ''))[:60]
    elif tool == 'TaskUpdate':
        desc = 'Task updated'
        tid = str(inp.get('taskId', ''))
        st = str(inp.get('status', ''))
        detail = '#' + tid + (' -> ' + st if st else '')
    elif tool == 'TaskList':
        desc = 'Listing tasks'
        detail = ''
    elif tool == 'TaskGet':
        desc = 'Getting task'
        detail = '#' + str(inp.get('taskId', ''))
    elif tool == 'ToolSearch':
        desc = 'Searching tools'
        detail = str(inp.get('query', ''))[:40]
    elif tool == 'AskUserQuestion':
        desc = 'Asking user'
        detail = str(inp.get('question', inp.get('message', '')))[:50]
    elif tool.startswith('mcp__'):
        parts = tool.split('__')
        mcp_name = parts[-1] if len(parts) > 1 else tool
        desc = 'MCP: ' + mcp_name.replace('_', ' ')[:30]
        detail = ''
    else:
        desc = tool
        detail = ''

    def esc(s):
        s = s.replace(chr(10), ' ').replace(chr(13), '')
        return s.replace(\"'\", \"'\\\"'\\\"'\")
    print(\"TOOL='%s'\" % esc(tool))
    print(\"FILE='%s'\" % esc(file_path))
    print(\"DESC='%s'\" % esc(desc))
    print(\"DETAIL='%s'\" % esc(detail))
    print(\"RESULT='%s'\" % esc(result))
    print(\"SUCCESS='%s'\" % ('1' if success else '0'))
except Exception as e:
    print(\"TOOL='unknown'\")
    print(\"FILE=''\")
    print(\"DESC='unknown'\")
    print(\"DETAIL=''\")
    print(\"RESULT=''\")
    print(\"SUCCESS='1'\")
" <<< "$input" 2>/dev/null)"

# Map tool to state
state="coding"
case "$TOOL" in
    Read)           state="reviewing" ;;
    Glob|Grep)      state="reviewing" ;;
    Edit|Write)     state="coding" ;;
    Bash)
        # Detect git commits
        if echo "$DETAIL" | grep -qi "git commit\|git push"; then
            state="committing"
        else
            state="coding"
        fi
        ;;
    Agent)          state="thinking" ;;
    Skill)          state="thinking" ;;
    WebSearch|WebFetch) state="reviewing" ;;
    *)              state="thinking" ;;
esac

# Get git branch
branch=$(git symbolic-ref --short HEAD 2>/dev/null || echo "")

# Fire and forget — include all enriched fields
"$HELIX_BIN" status-update \
    --cli claude-code \
    --state "$state" \
    --tool "$TOOL" \
    --file "$FILE" \
    --description "$DESC" \
    --detail "$DETAIL" \
    --result "$RESULT" \
    --success "$SUCCESS" \
    --cwd "$(pwd)" \
    --git-branch "$branch" &>/dev/null &
exit 0
