"""Memory compaction — archive, decay, flag, re-sync, backup.

Idempotent. Safe to run anytime, even concurrently with MCP calls.
Also writes health.json after compaction and logs to compaction.log.
"""

from __future__ import annotations

import logging
import math
from datetime import datetime, timedelta
from pathlib import Path

from helix_memory.core.backup import BackupManager
from helix_memory.core.embeddings import embed
from helix_memory.core.file_writer import atomic_write
from helix_memory.core.store import HelixStore
from helix_memory.core.world_state_gen import WorldStateGenerator
from helix_memory.utils.platform import get_log_dir, get_state_dir

logger = logging.getLogger(__name__)


class MemoryCompactor:
    """Compacts helix memory: archive, decay, flag, re-sync, backup."""

    def __init__(
        self,
        store: HelixStore,
        world_state_gen: WorldStateGenerator,
        backup_manager: BackupManager,
    ) -> None:
        self._store = store
        self._world_state_gen = world_state_gen
        self._backup_manager = backup_manager

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    async def compact(self, aggressive: bool = False) -> dict:
        """Run full compaction. Returns report dict.

        Steps:
        1. Archive old interactions (> 30 days, not already archived)
        2. Decay importance on inactive items
        3. Flag stale entities
        4. Resolve stale action items
        5. Re-sync denormalized names
        6. Regenerate world state (force=True)
        7. Export backup
        8. Write health.json
        9. Log to compaction.log
        10. (If aggressive) Prune archived interactions > 90 days
        """
        logger.info("Starting compaction (aggressive=%s)", aggressive)
        now = datetime.utcnow()

        archived_count = await self._step_archive_interactions(now)
        decayed_count = await self._step_decay_importance(now)
        stale_entities = await self._step_flag_stale_entities(now)
        stale_actions = await self._step_resolve_stale_actions(now)
        relations_synced = await self._step_resync_names()
        pruned_count = 0
        if aggressive:
            pruned_count = await self._step_prune_archived(now)

        # Regenerate world state
        await self._world_state_gen.generate(force=True)

        # Export backup
        backup_path = await self._backup_manager.export_backup()

        # Write health.json
        await self._write_health()

        report = {
            "archived_count": archived_count,
            "decayed_count": decayed_count,
            "stale_entities": stale_entities,
            "stale_actions": stale_actions,
            "relations_synced": relations_synced,
            "pruned_count": pruned_count,
            "backup_path": str(backup_path),
            "compacted_at": now.strftime("%Y-%m-%dT%H:%M:%SZ"),
        }

        # Log to compaction.log
        self._write_compaction_log(report)

        logger.info("Compaction complete: %s", report)
        return report

    # ------------------------------------------------------------------
    # Step implementations
    # ------------------------------------------------------------------

    async def _step_archive_interactions(self, now: datetime) -> int:
        """Archive interactions older than 30 days that aren't already archived."""
        cutoff = now - timedelta(days=30)
        old_interactions = await self._store.get_interactions_before(cutoff, archived=False)

        count = 0
        for interaction in old_interactions:
            interaction.archived = True
            interaction.archived_at = now
            interaction.raw_ref = ""
            # summary, entity_ids, entities, action_items are preserved
            interaction.prepare_for_save()
            vector = embed(interaction.summary)
            await self._store.upsert_interaction(interaction, vector)
            count += 1

        logger.info("Archived %d interactions", count)
        return count

    async def _step_decay_importance(self, now: datetime) -> int:
        """Decay importance on interactions inactive > 7 days. Floor at 0.1."""
        cutoff = now - timedelta(days=7)
        old_interactions = await self._store.get_interactions_before(cutoff, archived=False)

        count = 0
        for interaction in old_interactions:
            # Compute weeks inactive from timestamp
            age_days = (now - interaction.timestamp).days
            weeks_inactive = age_days / 7.0
            # importance *= 0.95^(weeks_inactive)
            decayed = interaction.importance * (0.95 ** weeks_inactive)
            new_importance = max(0.1, decayed)
            if new_importance != interaction.importance:
                interaction.importance = new_importance
                interaction.prepare_for_save()
                vector = embed(interaction.summary)
                await self._store.upsert_interaction(interaction, vector)
                count += 1

        logger.info("Decayed importance on %d interactions", count)
        return count

    async def _step_flag_stale_entities(self, now: datetime) -> int:
        """Add 'stale' tag to active entities inactive > 14 days."""
        cutoff = now - timedelta(days=14)
        stale = await self._store.get_entities_inactive_since(cutoff)

        count = 0
        for entity in stale:
            if "stale" not in entity.tags:
                entity.tags.append("stale")
                entity.prepare_for_save()
                vector = embed(entity.embedding_text)
                await self._store.upsert_entity(entity, vector)
                count += 1

        logger.info("Flagged %d stale entities", count)
        return count

    async def _step_resolve_stale_actions(self, now: datetime) -> int:
        """Set pending action items older than 30 days to status='stale'."""
        cutoff = now - timedelta(days=30)
        # Get all interactions with pending actions
        interactions_with_pending = await self._store.get_interactions_with_pending_actions()

        count = 0
        for interaction in interactions_with_pending:
            modified = False
            for action in interaction.action_items:
                if action.status in ("pending", "in_progress"):
                    # Check age from action's created_at
                    created = action.created_at
                    if isinstance(created, str):
                        try:
                            created = datetime.strptime(created, "%Y-%m-%dT%H:%M:%SZ")
                        except ValueError:
                            created = now
                    age_days = (now - created).days
                    if age_days > 30:
                        action.status = "stale"
                        action.resolved_at = now
                        modified = True
                        count += 1

            if modified:
                interaction.prepare_for_save()
                vector = embed(interaction.summary)
                await self._store.upsert_interaction(interaction, vector)

        logger.info("Resolved %d stale action items", count)
        return count

    async def _step_resync_names(self) -> int:
        """Re-sync denormalized target_name in entity relations."""
        all_entities = await self._store.list_all_entities(include_archived=True)

        # Build name map
        name_map: dict[str, str] = {e.id: e.name for e in all_entities}

        synced = 0
        for entity in all_entities:
            modified = False
            for rel in entity.relations:
                current_name = name_map.get(rel.target_id)
                if current_name is not None and current_name != rel.target_name:
                    rel.target_name = current_name
                    modified = True
                    synced += 1

            if modified:
                entity.prepare_for_save()
                vector = embed(entity.embedding_text)
                await self._store.upsert_entity(entity, vector)

        logger.info("Re-synced %d relation names", synced)
        return synced

    async def _step_prune_archived(self, now: datetime) -> int:
        """Hard delete archived interactions older than 90 days."""
        cutoff = now - timedelta(days=90)
        count = await self._store.delete_archived_before(cutoff)
        logger.info("Pruned %d archived interactions (> 90 days)", count)
        return count

    # ------------------------------------------------------------------
    # Health + logging
    # ------------------------------------------------------------------

    async def _write_health(self) -> None:
        """Write health status to ~/.helix/state/health.json"""
        now = datetime.utcnow()
        health = {
            "file_schema_version": 1,
            "status": "healthy",
            "qdrant_connected": True,
            "entity_count": await self._store.count_entities(),
            "interaction_count": await self._store.count_interactions(),
            "last_compaction": now.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "checked_at": now.strftime("%Y-%m-%dT%H:%M:%SZ"),
        }
        atomic_write(get_state_dir() / "health.json", health)
        logger.debug("health.json written")

    def _write_compaction_log(self, report: dict) -> None:
        """Append one line to ~/.helix/logs/compaction.log"""
        log_path = get_log_dir() / "compaction.log"
        import json as _json
        line = _json.dumps(report, default=str) + "\n"
        try:
            with log_path.open("a", encoding="utf-8") as f:
                f.write(line)
        except Exception:
            logger.warning("Failed to write compaction.log", exc_info=True)
