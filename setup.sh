#!/usr/bin/env bash
set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

ok()   { echo -e "  ${GREEN}[ok]${NC} $1"; }
warn() { echo -e "  ${YELLOW}[!!]${NC} $1"; }
fail() { echo -e "  ${RED}[err]${NC} $1"; }
info() { echo -e "  ${CYAN}[..]${NC} $1"; }

echo ""
echo -e "${BOLD}HELIX Setup${NC}"
echo "==========="
echo ""

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# -------------------------------------------------------------------
# 1. Check Rust toolchain
# -------------------------------------------------------------------
if command -v cargo &>/dev/null; then
    ok "Rust toolchain found ($(rustc --version 2>/dev/null | head -c 40))"
else
    warn "Rust not found"
    read -rp "    Install via rustup? [y/N] " yn
    if [[ "$yn" =~ ^[Yy]$ ]]; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env"
        ok "Rust installed"
    else
        fail "Rust is required for the TUI. Skipping TUI build."
    fi
fi

# -------------------------------------------------------------------
# 2. Build Helix TUI
# -------------------------------------------------------------------
if command -v cargo &>/dev/null; then
    info "Building Helix TUI (release)..."
    (cd "$SCRIPT_DIR/tui" && cargo build --release)
    ok "TUI built"

    # 3. Deploy binary
    DEPLOY_DIR="$HOME/.local/bin"
    mkdir -p "$DEPLOY_DIR"
    cp "$SCRIPT_DIR/tui/target/release/helix.exe" "$DEPLOY_DIR/helix.exe" 2>/dev/null \
        || cp "$SCRIPT_DIR/tui/target/release/helix" "$DEPLOY_DIR/helix" 2>/dev/null \
        || warn "Could not find helix binary — check build output"
    ok "Deployed to $DEPLOY_DIR"

    # Verify PATH
    if echo "$PATH" | tr ':' '\n' | grep -q "$(realpath "$DEPLOY_DIR")"; then
        ok "$DEPLOY_DIR is on PATH"
    else
        warn "$DEPLOY_DIR is not on PATH — add it to your shell profile"
    fi
else
    warn "Skipped TUI build (no Rust)"
fi

# -------------------------------------------------------------------
# 4. Copy hooks
# -------------------------------------------------------------------
HOOKS_DEST="$HOME/.claude/hooks"
if [ -d "$SCRIPT_DIR/hooks" ]; then
    mkdir -p "$HOOKS_DEST"
    cp "$SCRIPT_DIR/hooks/"* "$HOOKS_DEST/" 2>/dev/null && ok "Hooks copied to $HOOKS_DEST" \
        || warn "No hook files found"
else
    warn "hooks/ directory not found — skipping"
fi

# -------------------------------------------------------------------
# 5. Create runtime directory
# -------------------------------------------------------------------
mkdir -p "$HOME/.ai-status"
ok "Created ~/.ai-status/"

# -------------------------------------------------------------------
# 6. settings.json instructions
# -------------------------------------------------------------------
echo ""
echo -e "${BOLD}Hook Configuration${NC}"
echo ""
echo "  Add the following hooks to your Claude Code settings.json"
echo "  (~/.claude/settings.json):"
echo ""
echo '  "hooks": {'
echo '    "StatusLine": [{'
echo '      "type": "command",'
echo '      "command": "python ~/.claude/hooks/statusline-helix.py"'
echo '    }],'
echo '    "PostToolUse": [{'
echo '      "type": "command",'
echo '      "command": "bash ~/.claude/hooks/helix-post-tool.sh"'
echo '    }]'
echo '  }'
echo ""

# -------------------------------------------------------------------
# 7. Check Python
# -------------------------------------------------------------------
PYTHON=""
if command -v python3 &>/dev/null; then
    PYTHON="python3"
elif command -v python &>/dev/null; then
    PYTHON="python"
fi

if [ -n "$PYTHON" ]; then
    ok "Python found ($($PYTHON --version 2>&1))"
else
    warn "Python 3 not found — memory server requires Python 3.10+"
fi

# -------------------------------------------------------------------
# 8. Optionally install helix-memory
# -------------------------------------------------------------------
if [ -n "$PYTHON" ] && [ -d "$SCRIPT_DIR/memory" ]; then
    read -rp "  Install helix-memory (Python MCP server)? [y/N] " yn
    if [[ "$yn" =~ ^[Yy]$ ]]; then
        info "Installing helix-memory..."
        (cd "$SCRIPT_DIR/memory" && "$PYTHON" -m pip install -e .)
        ok "helix-memory installed"
    fi
fi

# -------------------------------------------------------------------
# 9. Optionally start Qdrant
# -------------------------------------------------------------------
if command -v docker &>/dev/null && [ -f "$SCRIPT_DIR/memory/docker-compose.yml" ]; then
    read -rp "  Start Qdrant via docker-compose? [y/N] " yn
    if [[ "$yn" =~ ^[Yy]$ ]]; then
        info "Starting Qdrant..."
        (cd "$SCRIPT_DIR/memory" && docker-compose up -d)
        ok "Qdrant running"
    fi
elif [ -f "$SCRIPT_DIR/memory/docker-compose.yml" ]; then
    warn "Docker not found — install Docker to run Qdrant"
fi

# -------------------------------------------------------------------
# Done
# -------------------------------------------------------------------
echo ""
echo -e "${GREEN}${BOLD}Setup complete.${NC}"
echo ""
echo "  Run the dashboard:  helix"
echo "  Run memory server:  cd memory && python -m helix_memory"
echo ""
