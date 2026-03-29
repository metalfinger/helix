# HELIX ⚡

> htop for your AI agents. Terminal dashboard + persistent memory for Claude Code, Codex, Gemini, and Aider.

<!-- screenshot placeholder - will add later -->

## What is Helix?

Two-part system:

1. **Helix TUI** — Rust terminal dashboard that monitors all AI coding sessions in real-time
2. **Helix Memory** — MCP server that gives your AI persistent memory across sessions

## Features

### Dashboard (tui/)

- Per-session cards: model, tokens, context%, git branch, state
- 8-color session palette (Cyan / Amber / Rose / Lime / Violet / Ice / Ember / Moss)
- Real-time activity feed with recency gradient
- Ambient effects: matrix rain (audio-reactive), breathing glow, fireflies, lava lamp, fractal plasma, cosmic eye
- 12 audio visualizer styles
- 3 themes: Cyberpunk, Clean, Retro
- Alert flash on high context (80% / 90%)
- Auto-detects new sessions

### Memory Server (memory/)

- 19 MCP tools for entities, memories, tasks, search
- Semantic search via Qdrant + sentence-transformers
- Entity system: projects, people, decisions with typed relations
- World state briefing: urgent items, deadlines, pending actions
- Time-decay on interactions, auto-archival
- Works with Claude Code, Cursor, Windsurf — anything that supports MCP

## Quick Start

### Option 1: Setup Script (recommended)

```bash
git clone https://github.com/metalfinger/helix.git
cd helix
bash setup.sh
```

### Option 2: Manual Setup

#### TUI Dashboard

```bash
cd tui
cargo build --release
cp target/release/helix.exe ~/.local/bin/  # or ~/bin/ and add to PATH
```

#### Memory Server

```bash
cd memory
pip install -e .
docker-compose up -d  # starts Qdrant
python -m helix_memory
```

#### Hooks

Copy hooks to `~/.claude/hooks/` and configure `settings.json`. See [hooks/](hooks/) for details.

## Demo Data

Want to see Helix in action without setting up your own data?

```bash
cd demo
python seed-demo-data.py
```

This populates Qdrant with sample projects, people, tasks, and decisions so you can explore the dashboard and memory system immediately.

## Architecture

```
Claude Code / Codex / Gemini session
  |-- statusLine hook --> statusline-helix.py --> ~/.ai-status/*.json
  |-- PostToolUse hook --> helix-post-tool.sh --> ~/.ai-status/*.json
  +-- MCP tools --> helix-memory --> Qdrant (vectors + entities)

Helix TUI watches ~/.ai-status/ and renders everything live
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `q` | Quit |
| `T` | Cycle theme |
| `R` | Matrix rain (audio-reactive) |
| `G` | Breathing glow |
| `F` | Fireflies |
| `L` | Lava lamp |
| `P` | Fractal plasma |
| `E` | Cosmic eye |
| `v` / `V` | Cycle / toggle visualizer |
| `A` | Activity overlay |

## Tech Stack

- **TUI**: Rust, Ratatui, Tokio, cpal (audio), sysinfo
- **Memory**: Python, FastMCP, Qdrant, sentence-transformers
- **Hooks**: Python + Bash (Claude Code integration)

## Contributing

PRs welcome. If you build something cool on top of Helix, let me know.

## License

MIT
