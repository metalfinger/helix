#!/usr/bin/env bash
# HELIX AfterTool hook for Gemini CLI
# Install in .gemini/settings.json:
#   { "hooks": { "AfterTool": [{ "command": "bash ~/.claude/hooks/helix-gemini-hook.sh" }] } }

command -v helix >/dev/null 2>&1 || exit 0

input=$(cat)

tool=$(python3 -c "
import json, sys
try:
    d = json.loads(sys.stdin.read())
    print(d.get('tool_name', d.get('toolName', '')))
except:
    print('unknown')
" <<< "$input" 2>/dev/null)

state="coding"
case "$tool" in
    read_file|search_files|list_directory) state="reviewing" ;;
    write_file|replace|edit_file)          state="coding" ;;
    run_shell_command|shell)               state="coding" ;;
    *)                                     state="thinking" ;;
esac

helix status-update --cli gemini --state "$state" --tool "$tool" --cwd "$(pwd)" &>/dev/null &
exit 0
