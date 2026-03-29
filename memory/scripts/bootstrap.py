"""Bootstrap helix-memory with seed entities from YAML.

Usage: python scripts/bootstrap.py [--yaml path/to/bootstrap.yaml]

Strategy (two-pass):
  Pass 1 — Create all entities without relations (handles forward references).
  Pass 2 — Resolve target names, build EntityRelation objects, re-upsert.
"""

from __future__ import annotations

import argparse
import asyncio
import sys
from pathlib import Path

import yaml
from rich.console import Console
from rich.table import Table

# Add project root to path
sys.path.insert(0, str(Path(__file__).parent.parent))

from helix_memory.config import settings
from helix_memory.core.embeddings import embed
from helix_memory.core.resolver import EntityExistsError, EntityResolver
from helix_memory.core.store import HelixStore
from helix_memory.models.entity import EntityRelation, WorldEntity

console = Console()

PROJECT_ROOT = Path(__file__).parent.parent
DEFAULT_YAML = PROJECT_ROOT / "config" / "bootstrap_entities.yaml"


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Bootstrap helix-memory seed entities.")
    parser.add_argument(
        "--yaml",
        type=Path,
        default=DEFAULT_YAML,
        metavar="PATH",
        help=f"Path to bootstrap YAML (default: {DEFAULT_YAML})",
    )
    return parser.parse_args()


def _load_yaml(path: Path) -> list[dict]:
    with path.open("r", encoding="utf-8") as f:
        data = yaml.safe_load(f)
    return data.get("entities", [])


async def _pass1_create_entities(
    resolver: EntityResolver,
    raw_entities: list[dict],
) -> dict[str, WorldEntity]:
    """Create all entities without relations. Returns name → WorldEntity map."""
    created: dict[str, WorldEntity] = {}
    skipped: list[str] = []
    warned: list[tuple[str, str]] = []

    console.print("\n[bold]Pass 1 — Creating entities[/bold]")

    for raw in raw_entities:
        name = raw["name"]
        entity = WorldEntity(
            type=raw["type"],
            name=name,
            aliases=raw.get("aliases", []),
            scope=raw.get("scope", "global"),
            priority=raw.get("priority", "medium"),
            context=raw.get("context", "").strip(),
            tags=raw.get("tags", []),
            relations=[],  # relations added in pass 2
        )
        try:
            saved, warning = await resolver.create_entity(entity)
            created[name] = saved
            console.print(f"  [green]CREATED[/green]  {name}")
            if warning:
                warned.append((name, warning))
                console.print(f"           [yellow]WARN[/yellow] {warning}")
        except EntityExistsError as exc:
            skipped.append(name)
            console.print(f"  [dim]SKIPPED[/dim]  {name}  ({exc})")

    return created


async def _pass2_add_relations(
    store: HelixStore,
    resolver: EntityResolver,
    raw_entities: list[dict],
    created_map: dict[str, WorldEntity],
) -> None:
    """Resolve relation targets and upsert each entity that has relations."""
    console.print("\n[bold]Pass 2 — Adding relations[/bold]")

    relation_count = 0
    skipped_relations: list[str] = []

    for raw in raw_entities:
        raw_relations = raw.get("relations", [])
        if not raw_relations:
            continue

        name = raw["name"]

        # Fetch fresh entity from store (may have been created in pass 1,
        # or already existed and was skipped)
        entity = await resolver.find_by_name_exact(name.lower().strip())
        if entity is None:
            console.print(f"  [red]MISSING[/red]  {name} — cannot add relations")
            continue

        new_relations: list[EntityRelation] = list(entity.relations)

        for rel_raw in raw_relations:
            target_name = rel_raw["target_name"]
            target = await resolver.find_by_name_exact(target_name.lower().strip())
            if target is None:
                skipped_relations.append(f"{name} → {target_name}")
                console.print(
                    f"  [yellow]WARN[/yellow]  Relation target '{target_name}' not found "
                    f"(from '{name}') — skipped"
                )
                continue

            # Check if relation already exists (idempotency)
            already = any(
                r.target_id == target.id and r.type == rel_raw["type"]
                for r in new_relations
            )
            if already:
                continue

            new_relations.append(
                EntityRelation(
                    target_id=target.id,
                    target_name=target.name,
                    type=rel_raw["type"],
                    detail=rel_raw.get("detail", ""),
                )
            )
            relation_count += 1

        # Only re-upsert if we actually added new relations
        if relation_count > 0 or new_relations != entity.relations:
            entity.relations = new_relations
            entity.prepare_for_save()
            vector = embed(entity.embedding_text)
            await store.upsert_entity(entity, vector)
            console.print(
                f"  [green]UPDATED[/green]  {name} — {len(new_relations)} relation(s)"
            )

    if skipped_relations:
        console.print(
            f"\n  [yellow]Skipped {len(skipped_relations)} relation(s) with missing targets.[/yellow]"
        )


async def main() -> None:
    args = _parse_args()

    if not args.yaml.exists():
        console.print(f"[red]ERROR[/red] YAML file not found: {args.yaml}")
        sys.exit(1)

    console.rule("[bold blue]helix-memory bootstrap[/bold blue]")
    console.print(f"YAML:    {args.yaml}")
    console.print(f"Qdrant:  {settings.qdrant.url}")

    raw_entities = _load_yaml(args.yaml)
    console.print(f"Loaded:  {len(raw_entities)} entity definition(s)")

    store = HelixStore(
        qdrant_url=settings.qdrant.url,
        collection_prefix=settings.qdrant.collection_prefix,
        api_key=settings.qdrant.api_key,
    )
    resolver = EntityResolver(store)

    try:
        created_map = await _pass1_create_entities(resolver, raw_entities)
        await _pass2_add_relations(store, resolver, raw_entities, created_map)
    finally:
        await store.close()

    # Summary table
    console.rule()
    table = Table(title="Bootstrap Summary", show_header=True)
    table.add_column("Metric", style="bold")
    table.add_column("Count", justify="right")
    table.add_row("Entities in YAML", str(len(raw_entities)))
    table.add_row("Created", str(len(created_map)))
    table.add_row("Skipped (already existed)", str(len(raw_entities) - len(created_map)))
    console.print(table)
    console.print("[green]Bootstrap complete.[/green]")


if __name__ == "__main__":
    asyncio.run(main())
