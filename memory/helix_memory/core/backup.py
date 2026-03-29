"""Backup and restore — JSON text export, no vectors.

Vectors are NOT backed up. They're recomputed from embedding_text (entities)
and summary (interactions) on restore. This keeps backups small and
survives embedding model changes.

latest.json is a file COPY (not symlink) — Windows compatibility.
"""

from __future__ import annotations

import json
import logging
import shutil
from datetime import datetime, timezone
from pathlib import Path

from helix_memory import __version__
from helix_memory.core.embeddings import get_model_name, embed
from helix_memory.core.file_writer import atomic_write
from helix_memory.core.store import HelixStore
from helix_memory.models.entity import WorldEntity
from helix_memory.models.interaction import Interaction
from helix_memory.utils.platform import get_backup_dir

logger = logging.getLogger(__name__)

SCHEMA_VERSION = 1


class BackupManager:
    """Export and restore helix memory to/from JSON backups."""

    def __init__(self, store: HelixStore) -> None:
        self._store = store

    async def export_backup(self) -> Path:
        """Export all entities and interactions to JSON.

        File: ~/.helix/backups/YYYY-MM-DD.json
        Also copies to ~/.helix/backups/latest.json

        Returns: path to backup file
        """
        from datetime import timedelta

        backup_dir = get_backup_dir()
        date_str = datetime.now(tz=timezone.utc).strftime("%Y-%m-%d")
        backup_path = backup_dir / f"{date_str}.json"
        latest_path = backup_dir / "latest.json"

        # Fetch all entities (including archived)
        entities = await self._store.list_all_entities(include_archived=True)

        # Fetch all interactions: active + archived
        interactions_active = await self._store.get_active_interactions()
        far_future = datetime.utcnow() + timedelta(days=365 * 100)
        interactions_archived = await self._store.get_interactions_before(far_future, archived=True)
        all_interactions = interactions_active + interactions_archived

        entity_dicts = [e.model_dump(mode="json") for e in entities]
        interaction_dicts = [i.model_dump(mode="json") for i in all_interactions]

        payload = {
            "helix_memory_version": __version__,
            "schema_version": SCHEMA_VERSION,
            "exported_at": datetime.now(tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "embedding_model": get_model_name(),
            "stats": {
                "entity_count": len(entity_dicts),
                "interaction_count": len(interaction_dicts),
            },
            "entities": entity_dicts,
            "interactions": interaction_dicts,
        }

        atomic_write(backup_path, payload)
        logger.info(
            "Backup written to %s (%d entities, %d interactions)",
            backup_path, len(entity_dicts), len(interaction_dicts),
        )

        # Copy to latest.json — file copy, not symlink (Windows compat)
        shutil.copy2(str(backup_path), str(latest_path))
        logger.debug("Copied backup to %s", latest_path)

        return backup_path

    async def restore_from_backup(self, backup_path: str | Path) -> dict:
        """Restore from JSON backup.

        Steps:
        1. Read JSON
        2. For each entity: reconstruct WorldEntity, call prepare_for_save(),
           embed(entity.embedding_text), upsert
        3. For each interaction: reconstruct Interaction, call prepare_for_save(),
           embed(interaction.summary), upsert
        4. Return stats dict

        Note: Entities embed on embedding_text field. Interactions embed on summary field.
        """
        backup_path = Path(backup_path)
        if not backup_path.exists():
            raise FileNotFoundError(f"Backup file not found: {backup_path}")

        raw = backup_path.read_text(encoding="utf-8")
        data = json.loads(raw)

        schema_ver = data.get("schema_version", 1)
        if schema_ver != SCHEMA_VERSION:
            raise ValueError(
                f"Unsupported backup schema version {schema_ver} "
                f"(expected {SCHEMA_VERSION})"
            )

        entity_dicts = data.get("entities", [])
        interaction_dicts = data.get("interactions", [])

        logger.info(
            "Restoring from %s: %d entities, %d interactions",
            backup_path, len(entity_dicts), len(interaction_dicts),
        )

        # Restore entities
        entity_errors = 0
        for raw_entity in entity_dicts:
            try:
                entity = WorldEntity.model_validate(raw_entity)
                entity.prepare_for_save()
                vector = embed(entity.embedding_text)
                await self._store.upsert_entity(entity, vector)
            except Exception:
                logger.exception("Failed to restore entity: %s", raw_entity.get("id"))
                entity_errors += 1

        # Restore interactions
        interaction_errors = 0
        for raw_interaction in interaction_dicts:
            try:
                interaction = Interaction.model_validate(raw_interaction)
                interaction.prepare_for_save()
                vector = embed(interaction.summary)
                await self._store.upsert_interaction(interaction, vector)
            except Exception:
                logger.exception("Failed to restore interaction: %s", raw_interaction.get("id"))
                interaction_errors += 1

        stats = {
            "entities_restored": len(entity_dicts) - entity_errors,
            "entity_errors": entity_errors,
            "interactions_restored": len(interaction_dicts) - interaction_errors,
            "interaction_errors": interaction_errors,
            "source_file": str(backup_path),
            "source_exported_at": data.get("exported_at"),
            "source_version": data.get("helix_memory_version"),
        }
        logger.info("Restore complete: %s", stats)
        return stats

    async def get_last_backup_info(self) -> dict | None:
        """Return info about latest backup (date, path, stats) or None if no backups."""
        backup_dir = get_backup_dir()
        latest_path = backup_dir / "latest.json"

        if not latest_path.exists():
            return None

        try:
            raw = latest_path.read_text(encoding="utf-8")
            data = json.loads(raw)
        except Exception:
            logger.exception("Failed to read latest.json")
            return None

        return {
            "path": str(latest_path),
            "exported_at": data.get("exported_at"),
            "helix_memory_version": data.get("helix_memory_version"),
            "embedding_model": data.get("embedding_model"),
            "stats": data.get("stats", {}),
        }
