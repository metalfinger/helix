"""Directory walker with gitignore + skip-list filtering."""

from __future__ import annotations

import os
from pathlib import Path
from typing import Iterator

try:
    import pathspec
except ImportError:
    pathspec = None  # type: ignore[assignment]

# Directories to always skip (case-insensitive on Windows)
SKIP_DIRS = frozenset({
    "node_modules", "target", "saved", "intermediate", "deriveddatacache",
    "__pycache__", ".git", "dist", "build", ".tox", ".mypy_cache",
    ".pytest_cache", ".venv", "venv", "env", ".next", ".nuxt",
    ".turbo", ".cache", ".svn", ".hg", "coverage", ".coverage",
    "htmlcov", ".eggs", "*.egg-info",
})

# File extensions to always skip
SKIP_EXTENSIONS = frozenset({
    ".uasset", ".umap", ".exe", ".dll", ".so", ".dylib", ".o", ".obj",
    ".pdb", ".lib", ".a", ".wasm", ".png", ".jpg", ".jpeg", ".gif",
    ".ico", ".svg", ".bmp", ".tiff", ".webp", ".mp3", ".mp4", ".wav",
    ".ogg", ".flac", ".avi", ".mkv", ".zip", ".tar", ".gz", ".7z",
    ".rar", ".bz2", ".xz", ".pdf", ".doc", ".docx", ".xls", ".xlsx",
    ".ppt", ".pptx", ".pyc", ".pyo", ".class", ".jar",
})

# Specific filenames to skip
SKIP_FILES = frozenset({
    "pnpm-lock.yaml", "package-lock.json", "yarn.lock", "cargo.lock",
    "poetry.lock", "composer.lock", "gemfile.lock", "flake.lock",
    ".ds_store", "thumbs.db", "desktop.ini",
})


def _load_gitignore(root: Path) -> "pathspec.PathSpec | None":
    """Load .gitignore from root directory, return a PathSpec matcher or None."""
    if pathspec is None:
        return None
    gitignore = root / ".gitignore"
    if not gitignore.is_file():
        return None
    try:
        text = gitignore.read_text(encoding="utf-8", errors="replace")
        return pathspec.PathSpec.from_lines("gitwildmatch", text.splitlines())
    except Exception:
        return None


def _is_binary(path: Path) -> bool:
    """Quick binary detection — check first 512 bytes for null bytes."""
    try:
        chunk = path.read_bytes()[:512]
        return b"\x00" in chunk
    except Exception:
        return True


def scan_project(root: str | Path) -> Iterator[Path]:
    """Walk project directory, yielding source file paths.

    Respects .gitignore, skips known non-source dirs/files/extensions,
    and detects binary files. Uses os.scandir for speed.
    """
    root = Path(root).resolve()
    gitignore = _load_gitignore(root)

    def _walk(directory: Path) -> Iterator[Path]:
        try:
            entries = list(os.scandir(directory))
        except PermissionError:
            return

        for entry in entries:
            name_lower = entry.name.lower()

            if entry.is_dir(follow_symlinks=False):
                if name_lower in SKIP_DIRS:
                    continue
                if name_lower.startswith(".") and name_lower != ".github":
                    continue
                # Check gitignore
                if gitignore:
                    rel = Path(entry.path).relative_to(root).as_posix() + "/"
                    if gitignore.match_file(rel):
                        continue
                yield from _walk(Path(entry.path))

            elif entry.is_file(follow_symlinks=False):
                # Skip by filename
                if name_lower in SKIP_FILES:
                    continue
                # Skip by extension
                ext = Path(name_lower).suffix
                if ext in SKIP_EXTENSIONS:
                    continue
                # Check gitignore
                if gitignore:
                    rel = Path(entry.path).relative_to(root).as_posix()
                    if gitignore.match_file(rel):
                        continue
                # Skip binary files
                p = Path(entry.path)
                if _is_binary(p):
                    continue
                yield p

    yield from _walk(root)
