"""
helix_memory.mcp.__main__

Allows running the MCP server via `python -m helix_memory.mcp`.
Delegates to server.main().
"""

from helix_memory.mcp.server import main

if __name__ == "__main__":
    main()
