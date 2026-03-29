"""Anatomy cache — JSON persistence with incremental updates."""

from __future__ import annotations

import hashlib
import json
import time
from pathlib import Path

from helix_memory.core.file_writer import atomic_write
from helix_memory.utils.platform import get_anatomy_dir


def project_hash(project_path: str) -> str:
    normalized = str(Path(project_path).resolve()).replace("\\", "/").lower()
    return hashlib.sha256(normalized.encode()).hexdigest()[:16]


def cache_path(project_path: str) -> Path:
    return get_anatomy_dir() / f"{project_hash(project_path)}.json"


def load_cache(project_path: str) -> dict | None:
    """Load cached anatomy. Returns None if missing."""
    cp = cache_path(project_path)
    if not cp.is_file():
        return None
    try:
        data = json.loads(cp.read_text(encoding="utf-8"))
        if data.get("version") != 1:
            return None
        return data
    except Exception:
        return None


def save_cache(anatomy: dict) -> Path:
    """Atomically write anatomy JSON to cache."""
    cp = cache_path(anatomy["project_path"])
    atomic_write(cp, anatomy)
    return cp


def is_stale(cached: dict, max_age_seconds: int = 3600) -> bool:
    """Check if cache is older than max_age_seconds."""
    scanned = cached.get("scanned_at", 0)
    if isinstance(scanned, str):
        return True  # can't parse, assume stale
    return (time.time() - scanned) > max_age_seconds
