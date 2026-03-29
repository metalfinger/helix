"""
helix_memory.utils.platform

Cross-platform helpers: data directory resolution, path normalisation, and
OS detection utilities used by config and file_writer. Populated in Epic 3a (E3a.1).
"""

import os
import subprocess
import sys
from pathlib import Path

_HELIX_ROOT = Path.home() / ".helix"


def _ensure(path: Path) -> Path:
    path.mkdir(parents=True, exist_ok=True)
    return path


def get_data_dir() -> Path:
    """Return ~/.helix/, creating it if missing."""
    return _ensure(_HELIX_ROOT)


def get_state_dir() -> Path:
    """Return ~/.helix/state/, creating if missing."""
    return _ensure(_HELIX_ROOT / "state")


def get_backup_dir() -> Path:
    """Return ~/.helix/backups/, creating if missing."""
    return _ensure(_HELIX_ROOT / "backups")


def get_log_dir() -> Path:
    """Return ~/.helix/logs/, creating if missing."""
    return _ensure(_HELIX_ROOT / "logs")


def get_anatomy_dir() -> Path:
    """Return ~/.helix/anatomy/, creating if missing."""
    return _ensure(_HELIX_ROOT / "anatomy")


def get_credentials_dir() -> Path:
    """Return ~/.helix/credentials/, creating if missing."""
    return _ensure(_HELIX_ROOT / "credentials")


def secure_file(path: Path) -> None:
    """Restrict file permissions to the current user only.

    Windows: uses icacls to remove inheritance and grant full control to %USERNAME%.
    Unix: chmod 600.
    """
    if sys.platform == "win32":
        username = os.environ.get("USERNAME", os.environ.get("USER", ""))
        subprocess.run(
            ["icacls", str(path), "/inheritance:r", "/grant:r", f"{username}:F"],
            check=True,
            capture_output=True,
        )
    else:
        os.chmod(path, 0o600)
