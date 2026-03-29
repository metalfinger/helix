#!/usr/bin/env bash
# HELIX status hook for Claude Code (secondary)
HELIX_BIN=$(command -v helix 2>/dev/null || echo "")
[[ -z "$HELIX_BIN" && -f "$HOME/bin/helix.exe" ]] && HELIX_BIN="$HOME/bin/helix.exe"
[[ -z "$HELIX_BIN" && -f "$HOME/bin/helix" ]] && HELIX_BIN="$HOME/bin/helix"
[[ -z "$HELIX_BIN" ]] && exit 0

input=$(cat)

eval "$(python3 -c "
import json, sys, os
try:
    d = json.loads(sys.stdin.read())
    tool = d.get('tool_name', '')
    inp = d.get('tool_input', {})
    if isinstance(inp, str):
        inp = json.loads(inp) if inp else {}
    file_path = str(inp.get('file_path', inp.get('command', inp.get('pattern', ''))) or '')[:120]
    desc = ''
    detail = ''
    if tool == 'Read':
        desc = 'Reading'
        detail = os.path.basename(file_path) if file_path else ''
    elif tool == 'Edit':
        desc = 'Editing'
        old_s = inp.get('old_string', '')
        new_s = inp.get('new_string', '')
        old_n = old_s.count(chr(10)) + (1 if old_s else 0)
        new_n = new_s.count(chr(10)) + (1 if new_s else 0)
        detail = '+%d -%d' % (new_n, old_n)
    elif tool == 'Write':
        desc = 'Creating'
        detail = os.path.basename(file_path) if file_path else ''
    elif tool == 'Bash':
        desc = inp.get('description', 'Running')[:60]
        cmd = inp.get('command', '')
        detail = cmd[:60]
    elif tool == 'Grep':
        desc = 'Searching'
        detail = '\"' + str(inp.get('pattern', ''))[:30] + '\"'
    elif tool == 'Glob':
        desc = 'Finding'
        detail = str(inp.get('pattern', ''))[:40]
    elif tool == 'Agent':
        desc = 'Spawning agent'
        detail = str(inp.get('prompt', inp.get('description', '')))[:40]
    else:
        desc = tool
        detail = ''
    def esc(s):
        return s.replace(\"'\", \"'\\\"'\\\"'\")
    print(\"TOOL='%s'\" % esc(tool))
    print(\"FILE='%s'\" % esc(file_path))
    print(\"DESC='%s'\" % esc(desc))
    print(\"DETAIL='%s'\" % esc(detail))
except:
    print(\"TOOL='unknown'\")
    print(\"FILE=''\")
    print(\"DESC='unknown'\")
    print(\"DETAIL=''\")
" <<< "$input" 2>/dev/null)"

state="coding"
case "$TOOL" in
    Read|Glob|Grep) state="reviewing" ;;
    Edit|Write)     state="coding" ;;
    Bash)           state="coding" ;;
    *)              state="thinking" ;;
esac

branch=$(git symbolic-ref --short HEAD 2>/dev/null || echo "")

"$HELIX_BIN" status-update --cli claude-code --state "$state" --tool "$TOOL" --file "$FILE" --description "$DESC" --detail "$DETAIL" --cwd "$(pwd)" --git-branch "$branch" &
disown
exit 0
