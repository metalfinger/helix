"""Search module — vector search + payload filtering.

Combines semantic search with Qdrant payload filters for precise results.
All query patterns from PRD Section 5.7.
"""

from __future__ import annotations

import asyncio
import logging
from datetime import datetime, timedelta, timezone
from typing import Optional

from qdrant_client import models

from helix_memory.core.embeddings import embed
from helix_memory.core.store import HelixStore
from helix_memory.models.entity import WorldEntity
from helix_memory.models.interaction import ActionItem, Interaction

logger = logging.getLogger(__name__)


class MemorySearch:
    """Semantic + filtered search over helix memory collections."""

    def __init__(self, store: HelixStore) -> None:
        self._store = store

    # ------------------------------------------------------------------
    # Public search methods
    # ------------------------------------------------------------------

    async def semantic_search(
        self,
        query: str,
        scope: str | None = None,
        entity_types: list[str] | None = None,
        time_range: str | None = None,
        limit: int = 5,
        score_threshold: float = 0.3,
    ) -> list[dict]:
        """Search across entities AND interactions.

        Returns list of dicts with keys:
        - type: "entity" or "interaction"
        - data: WorldEntity or Interaction
        - score: float
        - summary: str (entity name+context or interaction summary)

        Sorted by score descending.
        """
        vector = embed(query)

        entity_filter = self._build_entity_filter(
            scope=scope,
            entity_types=entity_types,
            status="active",
        )
        interaction_filter = self._build_interaction_filter(
            source=None,
            entity_id=None,
            time_range=time_range,
            archived=False,
        )

        entity_results, interaction_results = await asyncio.gather(
            self._store.vector_search_entities(
                vector=vector,
                filter_conditions=entity_filter,
                limit=limit,
                score_threshold=score_threshold,
            ),
            self._store.vector_search_interactions(
                vector=vector,
                filter_conditions=interaction_filter,
                limit=limit,
                score_threshold=score_threshold,
            ),
        )

        combined: list[dict] = []

        for entity, score in entity_results:
            combined.append(
                {
                    "type": "entity",
                    "data": entity,
                    "score": score,
                    "summary": f"{entity.name} — {entity.context}" if entity.context else entity.name,
                }
            )

        for interaction, score in interaction_results:
            combined.append(
                {
                    "type": "interaction",
                    "data": interaction,
                    "score": score,
                    "summary": interaction.summary,
                }
            )

        combined.sort(key=lambda r: r["score"], reverse=True)

        # Fallback: if vector search found nothing, try exact name/alias match
        # Short proper names like "Ravi" embed poorly and score below threshold
        if not combined:
            normalized = query.lower().strip()
            name_filter = models.Filter(
                should=[
                    models.FieldCondition(
                        key="name_normalized",
                        match=models.MatchValue(value=normalized),
                    ),
                    models.FieldCondition(
                        key="aliases_normalized",
                        match=models.MatchValue(value=normalized),
                    ),
                ],
            )
            name_results = await self._store.scroll_entities(
                filter_conditions=name_filter, limit=limit
            )
            for entity in name_results:
                combined.append(
                    {
                        "type": "entity",
                        "data": entity,
                        "score": 1.0,  # exact match
                        "summary": f"{entity.name} — {entity.context}" if entity.context else entity.name,
                    }
                )

        return combined[:limit]

    async def search_entities(
        self,
        query: str,
        scope: str | None = None,
        entity_types: list[str] | None = None,
        status: str = "active",
        limit: int = 5,
        score_threshold: float = 0.3,
    ) -> list[tuple[WorldEntity, float]]:
        """Search entities only. Returns (entity, score) pairs."""
        vector = embed(query)
        entity_filter = self._build_entity_filter(
            scope=scope,
            entity_types=entity_types,
            status=status,
        )
        return await self._store.vector_search_entities(
            vector=vector,
            filter_conditions=entity_filter,
            limit=limit,
            score_threshold=score_threshold,
        )

    async def search_interactions(
        self,
        query: str,
        source: str | None = None,
        entity_id: str | None = None,
        time_range: str | None = None,
        limit: int = 5,
        score_threshold: float = 0.3,
    ) -> list[tuple[Interaction, float]]:
        """Search interactions only. Returns (interaction, score) pairs."""
        vector = embed(query)
        interaction_filter = self._build_interaction_filter(
            source=source,
            entity_id=entity_id,
            time_range=time_range,
            archived=False,
        )
        return await self._store.vector_search_interactions(
            vector=vector,
            filter_conditions=interaction_filter,
            limit=limit,
            score_threshold=score_threshold,
        )

    # ------------------------------------------------------------------
    # Query helpers (filtered scroll, no vector search)
    # ------------------------------------------------------------------

    async def get_active_projects(self, scope: str | None = None) -> list[WorldEntity]:
        """All active projects, optionally filtered by scope."""
        entity_filter = self._build_entity_filter(
            scope=scope,
            entity_types=["project"],
            status="active",
        )
        return await self._store.scroll_entities(
            filter_conditions=entity_filter,
            limit=500,
        )

    async def get_pending_action_items(self) -> list[tuple[Interaction, list[ActionItem]]]:
        """All interactions with pending action items.

        Returns (interaction, pending_actions) pairs.
        """
        interactions = await self._store.get_interactions_with_pending_actions()
        results: list[tuple[Interaction, list[ActionItem]]] = []
        for interaction in interactions:
            pending = [
                a for a in interaction.action_items if a.status in ("pending", "in_progress")
            ]
            if pending:
                results.append((interaction, pending))
        return results

    async def get_stale_entities(self, days: int = 14) -> list[WorldEntity]:
        """Entities with no interaction in N days that are still active."""
        cutoff = datetime.utcnow() - timedelta(days=days)
        return await self._store.get_entities_inactive_since(cutoff)

    async def get_entity_timeline(self, entity_id: str, days: int = 14) -> list[Interaction]:
        """Chronological interactions for an entity, newest first."""
        cutoff = datetime.utcnow() - timedelta(days=days)
        cutoff_iso = cutoff.strftime("%Y-%m-%dT%H:%M:%SZ")

        scroll_filter = models.Filter(
            must=[
                models.FieldCondition(
                    key="entity_ids",
                    match=models.MatchValue(value=entity_id),
                ),
                models.FieldCondition(
                    key="timestamp",
                    range=models.DatetimeRange(gte=cutoff_iso),
                ),
                models.FieldCondition(
                    key="archived",
                    match=models.MatchValue(value=False),
                ),
            ]
        )

        records, _ = await self._store._client.scroll(
            collection_name=self._store._interactions_col,
            scroll_filter=scroll_filter,
            limit=500,
            with_payload=True,
            with_vectors=False,
        )
        parsed = [Interaction.model_validate(r.payload) for r in records]
        parsed.sort(key=lambda i: i.timestamp, reverse=True)
        return parsed

    # ------------------------------------------------------------------
    # Internal filter builders
    # ------------------------------------------------------------------

    def _build_entity_filter(
        self,
        scope: str | None,
        entity_types: list[str] | None,
        status: str | None,
    ) -> models.Filter | None:
        """Build a Qdrant Filter for entity queries."""
        conditions: list[models.Condition] = []

        if status:
            conditions.append(
                models.FieldCondition(
                    key="status",
                    match=models.MatchValue(value=status),
                )
            )

        if scope:
            conditions.append(
                models.FieldCondition(
                    key="scope",
                    match=models.MatchValue(value=scope),
                )
            )

        if entity_types:
            conditions.append(
                models.FieldCondition(
                    key="type",
                    match=models.MatchAny(any=entity_types),
                )
            )

        if not conditions:
            return None

        return models.Filter(must=conditions)

    def _build_interaction_filter(
        self,
        source: str | None,
        entity_id: str | None,
        time_range: str | None,
        archived: bool = False,
    ) -> models.Filter | None:
        """Build a Qdrant Filter for interaction queries."""
        conditions: list[models.Condition] = [
            models.FieldCondition(
                key="archived",
                match=models.MatchValue(value=archived),
            )
        ]

        if source:
            conditions.append(
                models.FieldCondition(
                    key="source",
                    match=models.MatchValue(value=source),
                )
            )

        if entity_id:
            conditions.append(
                models.FieldCondition(
                    key="entity_ids",
                    match=models.MatchValue(value=entity_id),
                )
            )

        if time_range:
            cutoff = self._time_range_to_datetime(time_range)
            cutoff_iso = cutoff.strftime("%Y-%m-%dT%H:%M:%SZ")
            conditions.append(
                models.FieldCondition(
                    key="timestamp",
                    range=models.DatetimeRange(gte=cutoff_iso),
                )
            )

        return models.Filter(must=conditions)

    def _time_range_to_datetime(self, time_range: str) -> datetime:
        """Convert time range string to a cutoff datetime (naive UTC).

        "today"      → midnight UTC today
        "this_week"  → 7 days ago
        "this_month" → 30 days ago
        """
        now = datetime.utcnow()

        if time_range == "today":
            return now.replace(hour=0, minute=0, second=0, microsecond=0)
        elif time_range == "this_week":
            return now - timedelta(days=7)
        elif time_range == "this_month":
            return now - timedelta(days=30)
        else:
            raise ValueError(
                f"Unknown time_range: {time_range!r}. Expected 'today', 'this_week', or 'this_month'."
            )
