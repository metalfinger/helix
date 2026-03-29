"""Startup health check for helix-memory.

Checks:
  1. [OK/FAIL] Qdrant connection
  2. [OK/FAIL] Collections exist (helix_entities, helix_interactions)
  3. [OK/FAIL] Vector dimensions match (384)
  4. [WARN]    All payload indexes present
  5. [WARN]    Schema version check
  6. [WARN]    World state freshness
  7. [INFO]    Entity count, interaction count
  8. [INFO]    Last backup date
  9. [OK]      Write health.json

On critical failure (1-3): exit with error
On warning (4-6): continue but warn
On info (7-9): print stats

Usage: python scripts/health_check.py
"""

from __future__ import annotations

import asyncio
import json
import sys
import tempfile
from datetime import datetime
from pathlib import Path

from rich.console import Console
from rich.table import Table

# Add project root to path
sys.path.insert(0, str(Path(__file__).parent.parent))

from helix_memory.config import settings
from helix_memory.core.store import HelixStore
from helix_memory.models.base import SCHEMA_VERSION

console = Console()

VECTOR_DIMENSIONS = 384

ENTITY_KEYWORD_INDEXES = [
    "type", "scope", "status", "priority",
    "name_normalized", "aliases_normalized", "tags",
]
ENTITY_DATETIME_INDEXES = ["updated_at", "last_interaction_at"]
INTERACTION_KEYWORD_INDEXES = ["source", "type", "entity_ids"]
INTERACTION_OTHER_INDEXES = ["timestamp", "importance", "archived", "has_pending_actions"]


def _now_iso() -> str:
    return datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")


def _status_icon(ok: bool) -> str:
    return "[green]OK  [/green]" if ok else "[red]FAIL[/red]"


def _warn_icon() -> str:
    return "[yellow]WARN[/yellow]"


def _info_icon() -> str:
    return "[cyan]INFO[/cyan]"


async def run_health_check() -> dict:
    """Run all checks and return a health report dict."""
    prefix = settings.qdrant.collection_prefix
    entities_col = f"{prefix}entities"
    interactions_col = f"{prefix}interactions"
    qdrant_url = settings.qdrant.url

    report: dict = {
        "timestamp": _now_iso(),
        "schema_version": SCHEMA_VERSION,
        "qdrant_url": qdrant_url,
        "checks": {},
        "stats": {},
        "warnings": [],
        "healthy": True,
    }

    critical_fail = False

    # ------------------------------------------------------------------
    # Import qdrant client directly for collection-level inspection
    # ------------------------------------------------------------------
    from qdrant_client import AsyncQdrantClient

    client = AsyncQdrantClient(url=qdrant_url, api_key=settings.qdrant.api_key)

    try:
        # 1. Qdrant connection
        console.print("\n[bold]Health Checks[/bold]")
        try:
            await client.get_collections()
            connected = True
        except Exception as exc:
            connected = False
            report["checks"]["qdrant_connection"] = {"ok": False, "error": str(exc)}
            console.print(f"  [{_status_icon(False)}] Qdrant connection — {exc}")
            critical_fail = True
        else:
            report["checks"]["qdrant_connection"] = {"ok": True}
            console.print(f"  [{_status_icon(True)}] Qdrant connection ({qdrant_url})")

        if critical_fail:
            report["healthy"] = False
            return report

        # 2. Collections exist
        entities_exists = await client.collection_exists(entities_col)
        interactions_exists = await client.collection_exists(interactions_col)
        collections_ok = entities_exists and interactions_exists

        report["checks"]["collections_exist"] = {
            "ok": collections_ok,
            entities_col: entities_exists,
            interactions_col: interactions_exists,
        }
        console.print(
            f"  [{_status_icon(entities_exists)}] Collection '{entities_col}' exists"
        )
        console.print(
            f"  [{_status_icon(interactions_exists)}] Collection '{interactions_col}' exists"
        )

        if not collections_ok:
            critical_fail = True
            report["healthy"] = False
            console.print(
                "  [red]Collections missing — run scripts/init_collections.py first.[/red]"
            )
            return report

        # 3. Vector dimensions
        ent_info = await client.get_collection(entities_col)
        int_info = await client.get_collection(interactions_col)

        ent_dim = ent_info.config.params.vectors.size if hasattr(ent_info.config.params.vectors, "size") else None
        int_dim = int_info.config.params.vectors.size if hasattr(int_info.config.params.vectors, "size") else None

        dims_ok = (ent_dim == VECTOR_DIMENSIONS) and (int_dim == VECTOR_DIMENSIONS)
        report["checks"]["vector_dimensions"] = {
            "ok": dims_ok,
            "expected": VECTOR_DIMENSIONS,
            entities_col: ent_dim,
            interactions_col: int_dim,
        }
        console.print(
            f"  [{_status_icon(dims_ok)}] Vector dimensions "
            f"(entities={ent_dim}, interactions={int_dim}, expected={VECTOR_DIMENSIONS})"
        )

        if not dims_ok:
            critical_fail = True
            report["healthy"] = False
            return report

        # 4. Payload indexes (WARN level)
        ent_indexes = set(ent_info.payload_schema.keys()) if ent_info.payload_schema else set()
        int_indexes = set(int_info.payload_schema.keys()) if int_info.payload_schema else set()

        expected_ent = set(ENTITY_KEYWORD_INDEXES + ENTITY_DATETIME_INDEXES)
        expected_int = set(INTERACTION_KEYWORD_INDEXES + INTERACTION_OTHER_INDEXES)

        missing_ent = expected_ent - ent_indexes
        missing_int = expected_int - int_indexes
        indexes_ok = not missing_ent and not missing_int

        report["checks"]["payload_indexes"] = {
            "ok": indexes_ok,
            "missing_entity_indexes": sorted(missing_ent),
            "missing_interaction_indexes": sorted(missing_int),
        }

        if indexes_ok:
            console.print(f"  [{_status_icon(True)}] Payload indexes present")
        else:
            msg = (
                f"Missing indexes — entities: {sorted(missing_ent)}, "
                f"interactions: {sorted(missing_int)}"
            )
            console.print(f"  [{_warn_icon()}] {msg}")
            report["warnings"].append(msg)

        # 5. Schema version check (sample a few entities)
        store = HelixStore(qdrant_url=qdrant_url, collection_prefix=prefix, api_key=settings.qdrant.api_key)
        try:
            sample = await store.scroll_entities(limit=10)
            outdated = [e for e in sample if e.schema_version < SCHEMA_VERSION]
            schema_warn = len(outdated) > 0
            report["checks"]["schema_version"] = {
                "ok": not schema_warn,
                "current": SCHEMA_VERSION,
                "outdated_in_sample": len(outdated),
                "sample_size": len(sample),
            }
            if schema_warn:
                msg = (
                    f"{len(outdated)}/{len(sample)} sampled entities have schema_version "
                    f"< {SCHEMA_VERSION} — run scripts/migrate.py"
                )
                console.print(f"  [{_warn_icon()}] {msg}")
                report["warnings"].append(msg)
            else:
                console.print(f"  [{_status_icon(True)}] Schema version (v{SCHEMA_VERSION})")

            # 6. World state freshness
            world_state_path = settings.data_dir / "world_state.json"
            stale_hours = settings.memory.world_state_stale_hours
            if world_state_path.exists():
                mtime = world_state_path.stat().st_mtime
                age_hours = (datetime.utcnow().timestamp() - mtime) / 3600
                fresh = age_hours <= stale_hours
                report["checks"]["world_state_freshness"] = {
                    "ok": fresh,
                    "age_hours": round(age_hours, 2),
                    "threshold_hours": stale_hours,
                    "path": str(world_state_path),
                }
                if fresh:
                    console.print(
                        f"  [{_status_icon(True)}] World state fresh "
                        f"({age_hours:.1f}h old, threshold={stale_hours}h)"
                    )
                else:
                    msg = (
                        f"World state is {age_hours:.1f}h old (threshold={stale_hours}h) "
                        "— consider regenerating"
                    )
                    console.print(f"  [{_warn_icon()}] {msg}")
                    report["warnings"].append(msg)
            else:
                msg = f"World state file not found at {world_state_path}"
                console.print(f"  [{_warn_icon()}] {msg}")
                report["checks"]["world_state_freshness"] = {"ok": False, "error": msg}
                report["warnings"].append(msg)

            # 7. Entity + interaction counts (INFO)
            entity_count = await store.count_entities()
            interaction_count = await store.count_interactions()
            report["stats"]["entity_count"] = entity_count
            report["stats"]["interaction_count"] = interaction_count
            console.print(
                f"  [{_info_icon()}] Entities: {entity_count}  |  "
                f"Interactions: {interaction_count}"
            )

            # 8. Last backup date (INFO)
            backup_dir = settings.data_dir / "backups"
            last_backup: str | None = None
            if backup_dir.exists():
                backups = sorted(backup_dir.glob("*.json.gz")) + sorted(backup_dir.glob("*.json"))
                if backups:
                    latest = backups[-1]
                    mtime = datetime.utcfromtimestamp(latest.stat().st_mtime)
                    last_backup = mtime.strftime("%Y-%m-%dT%H:%M:%SZ")
                    console.print(f"  [{_info_icon()}] Last backup: {last_backup} ({latest.name})")
                else:
                    console.print(f"  [{_info_icon()}] Last backup: none found in {backup_dir}")
            else:
                console.print(f"  [{_info_icon()}] Last backup: backup dir not found ({backup_dir})")

            report["stats"]["last_backup"] = last_backup

        finally:
            await store.close()

    finally:
        await client.close()

    if critical_fail:
        report["healthy"] = False

    return report


def _atomic_write(path: Path, data: dict) -> None:
    """Write JSON atomically via temp file + rename."""
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(".tmp")
    try:
        with tmp_path.open("w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, default=str)
        tmp_path.replace(path)
    except Exception:
        tmp_path.unlink(missing_ok=True)
        raise


async def main() -> None:
    console.rule("[bold blue]helix-memory health check[/bold blue]")
    console.print(f"Qdrant: {settings.qdrant.url}")
    console.print(f"Data dir: {settings.data_dir}")

    report = await run_health_check()

    # 9. Write health.json
    health_path = settings.data_dir / "state" / "health.json"
    try:
        _atomic_write(health_path, report)
        console.print(f"\n  [{_status_icon(True)}] health.json written -> {health_path}")
    except Exception as exc:
        console.print(f"\n  [red]FAIL[/red] Could not write health.json: {exc}")

    # Final verdict
    console.rule()
    if report["healthy"]:
        console.print("[green bold]HEALTHY[/green bold]")
    else:
        console.print("[red bold]UNHEALTHY — see errors above[/red bold]")
        sys.exit(1)

    if report["warnings"]:
        console.print(f"[yellow]{len(report['warnings'])} warning(s):[/yellow]")
        for w in report["warnings"]:
            console.print(f"  • {w}")


if __name__ == "__main__":
    asyncio.run(main())
