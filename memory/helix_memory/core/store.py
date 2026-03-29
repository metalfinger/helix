"""Qdrant store — CRUD operations for helix_entities and helix_interactions.

CRITICAL RULES (from PRD Section 5.9):
1. NEVER use partial payload updates. Always read full entity, modify in Python, upsert full payload.
2. ALWAYS call prepare_for_save() before upsert.
3. Full payload replacement via qdrant upsert. Last write wins.
"""

from __future__ import annotations

import logging
from datetime import datetime
from typing import Optional

from qdrant_client import AsyncQdrantClient
from qdrant_client import models

from helix_memory.models.entity import WorldEntity
from helix_memory.models.interaction import Interaction

logger = logging.getLogger(__name__)


def _to_qdrant_id(prefixed_id: str) -> str:
    """Convert prefixed ID (ent_<hex32> or int_<hex32>) to UUID string for Qdrant.

    Qdrant requires int or UUID point IDs. Our IDs are 'ent_' + 32-char hex
    which is exactly a UUID hex. We format it as UUID: 8-4-4-4-12.
    """
    hex_part = prefixed_id.split("_", 1)[1]
    # Format 32-char hex as UUID: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    return f"{hex_part[:8]}-{hex_part[8:12]}-{hex_part[12:16]}-{hex_part[16:20]}-{hex_part[20:]}"


class HelixStore:
    """Async Qdrant-backed store for entities and interactions."""

    def __init__(self, qdrant_url: str, collection_prefix: str = "helix_", api_key: str | None = None) -> None:
        self._client = AsyncQdrantClient(url=qdrant_url, api_key=api_key)
        self._entities_col = f"{collection_prefix}entities"
        self._interactions_col = f"{collection_prefix}interactions"

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _entity_from_record(self, record: models.Record) -> WorldEntity:
        return WorldEntity.model_validate(record.payload)

    def _interaction_from_record(self, record: models.Record) -> Interaction:
        return Interaction.model_validate(record.payload)

    def _entity_from_scored(self, point: models.ScoredPoint) -> WorldEntity:
        return WorldEntity.model_validate(point.payload)

    def _interaction_from_scored(self, point: models.ScoredPoint) -> Interaction:
        return Interaction.model_validate(point.payload)

    async def _scroll_all(
        self,
        collection_name: str,
        scroll_filter: models.Filter | None = None,
        batch_size: int = 100,
    ) -> list[models.Record]:
        """Scroll through all matching records in a collection."""
        results: list[models.Record] = []
        offset = None

        while True:
            batch, next_offset = await self._client.scroll(
                collection_name=collection_name,
                scroll_filter=scroll_filter,
                limit=batch_size,
                offset=offset,
                with_payload=True,
                with_vectors=False,
            )
            results.extend(batch)
            if next_offset is None:
                break
            offset = next_offset

        return results

    # ------------------------------------------------------------------
    # Entity methods
    # ------------------------------------------------------------------

    async def upsert_entity(self, entity: WorldEntity, vector: list[float]) -> None:
        """Full payload upsert. Always call entity.prepare_for_save() before this."""
        await self._client.upsert(
            collection_name=self._entities_col,
            points=[
                models.PointStruct(
                    id=_to_qdrant_id(entity.id),
                    vector=vector,
                    payload=entity.model_dump(mode="json"),
                )
            ],
        )

    async def delete_entity(self, entity_id: str) -> None:
        """Permanently delete an entity by its string ID."""
        await self._client.delete(
            collection_name=self._entities_col,
            points_selector=models.PointIdsList(points=[_to_qdrant_id(entity_id)]),
        )

    async def get_entity_by_id(self, entity_id: str) -> WorldEntity | None:
        """Retrieve a single entity by its string ID."""
        records = await self._client.retrieve(
            collection_name=self._entities_col,
            ids=[_to_qdrant_id(entity_id)],
            with_payload=True,
            with_vectors=False,
        )
        if not records:
            return None
        return self._entity_from_record(records[0])

    async def get_entities_by_ids(self, entity_ids: list[str]) -> list[WorldEntity]:
        """Retrieve multiple entities by their string IDs."""
        if not entity_ids:
            return []
        records = await self._client.retrieve(
            collection_name=self._entities_col,
            ids=[_to_qdrant_id(eid) for eid in entity_ids],
            with_payload=True,
            with_vectors=False,
        )
        return [self._entity_from_record(r) for r in records]

    async def scroll_entities(
        self,
        filter_conditions: models.Filter | None = None,
        limit: int = 50,
    ) -> list[WorldEntity]:
        """Return up to `limit` entities matching an optional filter."""
        batch, _ = await self._client.scroll(
            collection_name=self._entities_col,
            scroll_filter=filter_conditions,
            limit=limit,
            with_payload=True,
            with_vectors=False,
        )
        return [self._entity_from_record(r) for r in batch]

    async def list_all_entities(self, include_archived: bool = False) -> list[WorldEntity]:
        """Return all entities, optionally excluding archived ones."""
        if include_archived:
            scroll_filter = None
        else:
            scroll_filter = models.Filter(
                must_not=[
                    models.FieldCondition(
                        key="status",
                        match=models.MatchValue(value="archived"),
                    )
                ]
            )

        records = await self._scroll_all(self._entities_col, scroll_filter=scroll_filter)
        return [self._entity_from_record(r) for r in records]

    async def get_entities_inactive_since(self, cutoff: datetime) -> list[WorldEntity]:
        """Return active entities whose last_interaction_at is before cutoff."""
        cutoff_iso = cutoff.strftime("%Y-%m-%dT%H:%M:%SZ")
        scroll_filter = models.Filter(
            must=[
                models.FieldCondition(
                    key="status",
                    match=models.MatchValue(value="active"),
                ),
                models.FieldCondition(
                    key="last_interaction_at",
                    range=models.DatetimeRange(lt=cutoff_iso),
                ),
            ]
        )
        records = await self._scroll_all(self._entities_col, scroll_filter=scroll_filter)
        return [self._entity_from_record(r) for r in records]

    # ------------------------------------------------------------------
    # Interaction methods
    # ------------------------------------------------------------------

    async def upsert_interaction(self, interaction: Interaction, vector: list[float]) -> None:
        """Full payload upsert. Always call interaction.prepare_for_save() before this."""
        await self._client.upsert(
            collection_name=self._interactions_col,
            points=[
                models.PointStruct(
                    id=_to_qdrant_id(interaction.id),
                    vector=vector,
                    payload=interaction.model_dump(mode="json"),
                )
            ],
        )

    async def get_interactions_before(
        self, cutoff: datetime, archived: bool = False
    ) -> list[Interaction]:
        """Return interactions whose timestamp is before cutoff."""
        cutoff_iso = cutoff.strftime("%Y-%m-%dT%H:%M:%SZ")
        must = [
            models.FieldCondition(
                key="timestamp",
                range=models.DatetimeRange(lt=cutoff_iso),
            ),
            models.FieldCondition(
                key="archived",
                match=models.MatchValue(value=archived),
            ),
        ]
        scroll_filter = models.Filter(must=must)
        records = await self._scroll_all(self._interactions_col, scroll_filter=scroll_filter)
        return [self._interaction_from_record(r) for r in records]

    async def get_active_interactions(self) -> list[Interaction]:
        """Return all non-archived interactions."""
        scroll_filter = models.Filter(
            must=[
                models.FieldCondition(
                    key="archived",
                    match=models.MatchValue(value=False),
                )
            ]
        )
        records = await self._scroll_all(self._interactions_col, scroll_filter=scroll_filter)
        return [self._interaction_from_record(r) for r in records]

    async def get_interactions_for_entity(
        self, entity_id: str, limit: int = 20
    ) -> list[Interaction]:
        """Return interactions that mention a specific entity."""
        scroll_filter = models.Filter(
            must=[
                models.FieldCondition(
                    key="entity_ids",
                    match=models.MatchValue(value=entity_id),
                )
            ]
        )
        batch, _ = await self._client.scroll(
            collection_name=self._interactions_col,
            scroll_filter=scroll_filter,
            limit=limit,
            with_payload=True,
            with_vectors=False,
        )
        return [self._interaction_from_record(r) for r in batch]

    async def get_interactions_with_pending_actions(self) -> list[Interaction]:
        """Return non-archived interactions that have pending action items."""
        scroll_filter = models.Filter(
            must=[
                models.FieldCondition(
                    key="has_pending_actions",
                    match=models.MatchValue(value=True),
                ),
                models.FieldCondition(
                    key="archived",
                    match=models.MatchValue(value=False),
                ),
            ]
        )
        records = await self._scroll_all(self._interactions_col, scroll_filter=scroll_filter)
        return [self._interaction_from_record(r) for r in records]

    async def get_recent_interactions(self, hours: int = 48) -> list[Interaction]:
        """Return interactions from the last N hours."""
        from datetime import timedelta

        cutoff = datetime.utcnow() - timedelta(hours=hours)
        cutoff_iso = cutoff.strftime("%Y-%m-%dT%H:%M:%SZ")
        scroll_filter = models.Filter(
            must=[
                models.FieldCondition(
                    key="timestamp",
                    range=models.DatetimeRange(gte=cutoff_iso),
                )
            ]
        )
        records = await self._scroll_all(self._interactions_col, scroll_filter=scroll_filter)
        return [self._interaction_from_record(r) for r in records]

    async def delete_archived_before(self, cutoff: datetime) -> int:
        """Hard delete archived interactions older than cutoff. Returns count deleted."""
        cutoff_iso = cutoff.strftime("%Y-%m-%dT%H:%M:%SZ")
        delete_filter = models.Filter(
            must=[
                models.FieldCondition(
                    key="archived",
                    match=models.MatchValue(value=True),
                ),
                models.FieldCondition(
                    key="timestamp",
                    range=models.DatetimeRange(lt=cutoff_iso),
                ),
            ]
        )

        # Count before delete
        count_result = await self._client.count(
            collection_name=self._interactions_col,
            count_filter=delete_filter,
            exact=True,
        )
        n = count_result.count

        if n > 0:
            await self._client.delete(
                collection_name=self._interactions_col,
                points_selector=models.FilterSelector(filter=delete_filter),
            )

        return n

    # ------------------------------------------------------------------
    # Vector search methods
    # ------------------------------------------------------------------

    async def vector_search_entities(
        self,
        vector: list[float],
        filter_conditions: models.Filter | None = None,
        limit: int = 5,
        score_threshold: float = 0.0,
    ) -> list[tuple[WorldEntity, float]]:
        """Semantic search over entities. Returns (entity, score) pairs."""
        response = await self._client.query_points(
            collection_name=self._entities_col,
            query=vector,
            query_filter=filter_conditions,
            limit=limit,
            score_threshold=score_threshold if score_threshold > 0.0 else None,
            with_payload=True,
            with_vectors=False,
        )
        return [(self._entity_from_scored(p), p.score) for p in response.points]

    async def vector_search_interactions(
        self,
        vector: list[float],
        filter_conditions: models.Filter | None = None,
        limit: int = 5,
        score_threshold: float = 0.0,
    ) -> list[tuple[Interaction, float]]:
        """Semantic search over interactions. Returns (interaction, score) pairs."""
        response = await self._client.query_points(
            collection_name=self._interactions_col,
            query=vector,
            query_filter=filter_conditions,
            limit=limit,
            score_threshold=score_threshold if score_threshold > 0.0 else None,
            with_payload=True,
            with_vectors=False,
        )
        return [(self._interaction_from_scored(p), p.score) for p in response.points]

    # ------------------------------------------------------------------
    # Count methods
    # ------------------------------------------------------------------

    async def count_entities(self) -> int:
        """Return total number of entity points in the collection."""
        result = await self._client.count(
            collection_name=self._entities_col,
            exact=True,
        )
        return result.count

    async def count_interactions(self) -> int:
        """Return total number of interaction points in the collection."""
        result = await self._client.count(
            collection_name=self._interactions_col,
            exact=True,
        )
        return result.count

    # ------------------------------------------------------------------
    # Connection
    # ------------------------------------------------------------------

    async def close(self) -> None:
        """Close the underlying Qdrant client connection."""
        await self._client.close()
