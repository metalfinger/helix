"""Schema migration — upgrade data points to current schema version.

Scrolls all entity and interaction points whose schema_version < SCHEMA_VERSION,
applies schema defaults, re-embeds if the embedding_text field is missing or
the embedding_model has changed, and upserts the corrected payload.

Usage: python scripts/migrate.py
"""

from __future__ import annotations

import asyncio
import sys
from datetime import datetime
from pathlib import Path

from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn, BarColumn, TaskProgressColumn

# Add project root to path
sys.path.insert(0, str(Path(__file__).parent.parent))

from helix_memory.config import settings
from helix_memory.core.embeddings import embed, get_model_name, EMBEDDING_DIMENSIONS
from helix_memory.core.store import HelixStore
from helix_memory.models.base import SCHEMA_VERSION, EMBEDDING_MODEL, EMBEDDING_VERSION
from helix_memory.models.entity import WorldEntity
from helix_memory.models.interaction import Interaction

console = Console()


# ------------------------------------------------------------------
# Migration functions — one per model type
# ------------------------------------------------------------------

def _migrate_entity(entity: WorldEntity) -> tuple[WorldEntity, bool]:
    """Apply defaults for any missing fields introduced in later schema versions.

    Returns (entity, needs_re_embed).
    """
    needs_re_embed = False

    # v1 baseline: ensure all required fields have defaults
    if not entity.embedding_text:
        entity.prepare_for_save()
        needs_re_embed = True

    if entity.embedding_model != EMBEDDING_MODEL:
        # Model changed — must re-embed
        entity.embedding_model = EMBEDDING_MODEL
        entity.embedding_version = EMBEDDING_VERSION
        needs_re_embed = True

    if not entity.name_normalized:
        entity.name_normalized = entity.name.lower().strip()

    if entity.aliases_normalized != [a.lower().strip() for a in entity.aliases]:
        entity.aliases_normalized = [a.lower().strip() for a in entity.aliases]

    entity.schema_version = SCHEMA_VERSION
    entity.updated_at = datetime.utcnow()

    return entity, needs_re_embed


def _migrate_interaction(interaction: Interaction) -> tuple[Interaction, bool]:
    """Apply defaults for any missing fields introduced in later schema versions.

    Returns (interaction, needs_re_embed).
    """
    needs_re_embed = False

    if interaction.embedding_model != EMBEDDING_MODEL:
        interaction.embedding_model = EMBEDDING_MODEL
        # embedding_version is present on Interaction via HelixBase field inheritance
        if hasattr(interaction, "embedding_version"):
            interaction.embedding_version = EMBEDDING_VERSION
        needs_re_embed = True

    interaction.schema_version = SCHEMA_VERSION

    return interaction, needs_re_embed


# ------------------------------------------------------------------
# Main migration runner
# ------------------------------------------------------------------

async def migrate_entities(store: HelixStore) -> tuple[int, int]:
    """Migrate all outdated entities. Returns (total_found, migrated)."""
    from qdrant_client import models

    outdated_filter = models.Filter(
        must=[
            models.FieldCondition(
                key="schema_version",
                range=models.Range(lt=SCHEMA_VERSION),
            )
        ]
    )

    # Use store's internal client to scroll all points with outdated schema
    # We pull them via list_all_entities with a manual version check since
    # HelixStore doesn't expose a raw scroll with arbitrary filter.
    all_entities = await store.list_all_entities(include_archived=True)
    outdated = [e for e in all_entities if e.schema_version < SCHEMA_VERSION]

    total = len(outdated)
    if total == 0:
        console.print("  [green]Entities: all up to date.[/green]")
        return 0, 0

    migrated = 0
    errors = 0

    with Progress(
        SpinnerColumn(),
        TextColumn("[progress.description]{task.description}"),
        BarColumn(),
        TaskProgressColumn(),
        console=console,
        transient=True,
    ) as progress:
        task = progress.add_task(f"Migrating {total} entities …", total=total)

        for entity in outdated:
            try:
                entity, needs_re_embed = _migrate_entity(entity)

                if needs_re_embed:
                    entity.prepare_for_save()
                    vector = embed(entity.embedding_text)
                else:
                    # Still need a vector for upsert — re-embed from existing text
                    vector = embed(entity.embedding_text or entity.name)

                await store.upsert_entity(entity, vector)
                migrated += 1
            except Exception as exc:
                errors += 1
                console.print(f"  [red]ERROR[/red] Entity '{entity.name}': {exc}")

            progress.advance(task)

    console.print(
        f"  Entities: [green]{migrated} migrated[/green]"
        + (f", [red]{errors} errors[/red]" if errors else "")
        + f" (of {total} outdated)"
    )
    return total, migrated


async def migrate_interactions(store: HelixStore) -> tuple[int, int]:
    """Migrate all outdated interactions. Returns (total_found, migrated)."""
    all_interactions = await store.get_active_interactions()
    # Also grab archived
    from datetime import timedelta
    archived = await store.get_interactions_before(
        datetime.utcnow() + timedelta(days=36500), archived=True
    )
    all_interactions = all_interactions + archived

    outdated = [i for i in all_interactions if i.schema_version < SCHEMA_VERSION]

    total = len(outdated)
    if total == 0:
        console.print("  [green]Interactions: all up to date.[/green]")
        return 0, 0

    migrated = 0
    errors = 0

    with Progress(
        SpinnerColumn(),
        TextColumn("[progress.description]{task.description}"),
        BarColumn(),
        TaskProgressColumn(),
        console=console,
        transient=True,
    ) as progress:
        task = progress.add_task(f"Migrating {total} interactions …", total=total)

        for interaction in outdated:
            try:
                interaction, needs_re_embed = _migrate_interaction(interaction)

                # Interactions embed their summary field
                embed_text = interaction.summary or str(interaction.id)
                vector = embed(embed_text)
                await store.upsert_interaction(interaction, vector)
                migrated += 1
            except Exception as exc:
                errors += 1
                console.print(f"  [red]ERROR[/red] Interaction '{interaction.id}': {exc}")

            progress.advance(task)

    console.print(
        f"  Interactions: [green]{migrated} migrated[/green]"
        + (f", [red]{errors} errors[/red]" if errors else "")
        + f" (of {total} outdated)"
    )
    return total, migrated


async def main() -> None:
    console.rule("[bold blue]helix-memory schema migration[/bold blue]")
    console.print(f"Target schema version: [bold]{SCHEMA_VERSION}[/bold]")
    console.print(f"Embedding model:       {EMBEDDING_MODEL}")
    console.print(f"Qdrant:                {settings.qdrant.url}")
    console.print()

    store = HelixStore(
        qdrant_url=settings.qdrant.url,
        collection_prefix=settings.qdrant.collection_prefix,
        api_key=settings.qdrant.api_key,
    )

    try:
        console.print("[bold]Entities[/bold]")
        ent_total, ent_migrated = await migrate_entities(store)

        console.print("[bold]Interactions[/bold]")
        int_total, int_migrated = await migrate_interactions(store)
    finally:
        await store.close()

    console.rule()
    if ent_total == 0 and int_total == 0:
        console.print("[green]Nothing to migrate — all data is current.[/green]")
    else:
        console.print(
            f"[green]Migration complete.[/green] "
            f"Entities: {ent_migrated}/{ent_total}  "
            f"Interactions: {int_migrated}/{int_total}"
        )


if __name__ == "__main__":
    asyncio.run(main())
