"""High-level anatomy API — scan, cache, query."""

from __future__ import annotations

import time
from pathlib import Path

from helix_memory.anatomy.scanner import scan_project
from helix_memory.anatomy.extractor import extract_file_info
from helix_memory.anatomy.cache import load_cache, save_cache, is_stale


def _build_anatomy(project_path: str) -> dict:
    """Full scan of a project directory."""
    root = Path(project_path).resolve()
    files = {}
    total_tokens = 0

    for fpath in scan_project(root):
        rel = fpath.relative_to(root).as_posix()
        info = extract_file_info(fpath)
        files[rel] = info
        total_tokens += info["tokens"]

    return {
        "version": 1,
        "project_path": str(root),
        "scanned_at": time.time(),
        "file_count": len(files),
        "total_tokens": total_tokens,
        "files": files,
    }


def _incremental_update(cached: dict, project_path: str) -> dict:
    """Update only changed/new files, remove deleted ones."""
    root = Path(project_path).resolve()
    current_files = set()
    files = dict(cached.get("files", {}))
    changed = 0

    for fpath in scan_project(root):
        rel = fpath.relative_to(root).as_posix()
        current_files.add(rel)

        try:
            mtime = fpath.stat().st_mtime
        except OSError:
            continue

        existing = files.get(rel)
        if existing and existing.get("last_modified", 0) >= mtime:
            continue  # unchanged

        # Re-extract
        files[rel] = extract_file_info(fpath)
        changed += 1

    # Remove deleted files
    deleted = set(files.keys()) - current_files
    for d in deleted:
        del files[d]
        changed += 1

    total_tokens = sum(f["tokens"] for f in files.values())

    return {
        "version": 1,
        "project_path": str(root),
        "scanned_at": time.time(),
        "file_count": len(files),
        "total_tokens": total_tokens,
        "files": files,
    }


def _filter_files(anatomy: dict, query: str) -> dict:
    """Filter anatomy files by query string (substring match on path, description, symbols)."""
    q = query.lower()
    matched = {}
    for rel, info in anatomy["files"].items():
        if q in rel.lower():
            matched[rel] = info
            continue
        if q in info.get("description", "").lower():
            matched[rel] = info
            continue
        if q in info.get("language", "").lower():
            matched[rel] = info
            continue
        if any(q in s.lower() for s in info.get("symbols", [])):
            matched[rel] = info
            continue

    return matched


def _format_output(anatomy: dict, files: dict | None = None, query: str | None = None) -> str:
    """Format anatomy as readable markdown output for Claude."""
    if files is None:
        files = anatomy["files"]

    # Language breakdown
    lang_counts: dict[str, int] = {}
    lang_tokens: dict[str, int] = {}
    for info in files.values():
        lang = info.get("language", "unknown")
        lang_counts[lang] = lang_counts.get(lang, 0) + 1
        lang_tokens[lang] = lang_tokens.get(lang, 0) + info["tokens"]

    total_tok = sum(f["tokens"] for f in files.values())
    lines = []

    if query:
        lines.append(f"## Anatomy — {len(files)} files matching \"{query}\" ({total_tok:,} tokens)")
    else:
        lines.append(f"## Anatomy — {len(files)} files ({total_tok:,} tokens)")

    # Language summary
    lang_parts = []
    for lang in sorted(lang_counts, key=lambda l: lang_tokens[l], reverse=True):
        lang_parts.append(f"{lang}: {lang_counts[lang]} files ({lang_tokens[lang]:,} tok)")
    lines.append("Languages: " + ", ".join(lang_parts[:8]))
    lines.append("")

    # Group files by top-level directory
    groups: dict[str, list[tuple[str, dict]]] = {}
    for rel, info in sorted(files.items()):
        parts = rel.split("/")
        group = parts[0] if len(parts) > 1 else "."
        groups.setdefault(group, []).append((rel, info))

    for group in sorted(groups):
        entries = groups[group]
        group_tok = sum(info["tokens"] for _, info in entries)
        lines.append(f"### {group}/ ({len(entries)} files, {group_tok:,} tok)")

        for rel, info in entries:
            symbols_str = ""
            if info.get("symbols"):
                syms = ", ".join(info["symbols"][:8])
                if len(info["symbols"]) > 8:
                    syms += f" +{len(info['symbols']) - 8} more"
                symbols_str = f" [{syms}]"

            lines.append(
                f"  {rel:<50} {info['tokens']:>6} tok  — {info['description']}{symbols_str}"
            )

        lines.append("")

    return "\n".join(lines)


async def get_anatomy(
    project_path: str | None = None,
    query: str | None = None,
    path_filter: str | None = None,
    force_rescan: bool = False,
) -> str:
    """Main entry point — get anatomy map for a project.

    Args:
        project_path: Root directory. Defaults to CWD.
        query: Optional filter string.
        path_filter: Optional path prefix filter (e.g. "src/auth").
        force_rescan: Skip cache, do full rescan.

    Returns:
        Formatted markdown string.
    """
    import os
    if project_path is None:
        project_path = os.getcwd()

    project_path = str(Path(project_path).resolve())

    # Try cache
    anatomy = None
    if not force_rescan:
        cached = load_cache(project_path)
        if cached and not is_stale(cached):
            anatomy = cached
        elif cached:
            # Incremental update
            anatomy = _incremental_update(cached, project_path)
            save_cache(anatomy)

    if anatomy is None:
        anatomy = _build_anatomy(project_path)
        save_cache(anatomy)

    # Apply filters
    files = anatomy["files"]

    if path_filter:
        pf = path_filter.replace("\\", "/").strip("/")
        files = {k: v for k, v in files.items() if k.startswith(pf)}

    if query:
        files = _filter_files({"files": files}, query)

    # Cap output at 200 files
    if len(files) > 200:
        # Sort by tokens descending, keep top 200
        sorted_files = sorted(files.items(), key=lambda x: x[1]["tokens"], reverse=True)[:200]
        files = dict(sorted_files)

    return _format_output(anatomy, files, query)
