"""
helix_memory.core.file_writer

Atomic file-write helpers for persisting world-state and config snapshots to
~/.helix/. Uses temp-file + rename for crash safety. Populated in Epic 3a.
"""

"""Atomic file writer — write to temp, then os.replace() for atomicity on NTFS/ext4."""

import json
import os
from pathlib import Path
from typing import Any


def atomic_write(path: str | Path, data: str | dict | Any, *, indent: int = 2) -> None:
    """Write data atomically to path.

    Strategy: write to {path}.tmp in same directory, then os.replace().
    os.replace() is atomic on NTFS (Windows) and POSIX for same-volume renames.

    Args:
        path: Target file path
        data: String content, or dict/object to serialize as JSON
        indent: JSON indent (only used if data is dict/object)
    """
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(path.suffix + ".tmp")

    try:
        content = data if isinstance(data, str) else json.dumps(data, indent=indent, default=str)
        tmp_path.write_text(content, encoding="utf-8")
        os.replace(str(tmp_path), str(path))
    except Exception:
        # Clean up temp file on failure
        if tmp_path.exists():
            tmp_path.unlink()
        raise
