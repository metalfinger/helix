"""Entity resolution — deduplication on creation, fuzzy matching on lookup.

Prevents the #1 knowledge graph failure: "Ravi" vs "ravi" vs "Ravi K" becoming 3 entities.
"""

from __future__ import annotations

import logging

from qdrant_client import models

from helix_memory.core.embeddings import embed
from helix_memory.core.store import HelixStore
from helix_memory.models.entity import WorldEntity

logger = logging.getLogger(__name__)


class EntityExistsError(Exception):
    """Raised when entity creation would create a duplicate."""
    pass


class EntityResolver:
    """Resolves and deduplicates entities against the Qdrant store."""

    def __init__(self, store: HelixStore) -> None:
        self._store = store

    # ------------------------------------------------------------------
    # Creation with deduplication
    # ------------------------------------------------------------------

    async def create_entity(self, entity: WorldEntity) -> tuple[WorldEntity, str | None]:
        """Create entity with deduplication checks.

        Returns: (created_entity, warning_message_or_None)
        Raises: EntityExistsError if exact name or alias collision

        Steps:
        1. Exact name match → raise EntityExistsError
        2. Alias collision → raise EntityExistsError
        3. Fuzzy similarity (>0.85) → create but return warning
        4. Store with embedding
        """
        name_normalized = entity.name.lower().strip()

        # Step 1: exact name match
        existing = await self.find_by_name_exact(name_normalized)
        if existing is not None:
            raise EntityExistsError(
                f"Entity with name '{entity.name}' already exists (id={existing.id})"
            )

        # Step 2: alias collision — check each alias on the incoming entity
        # against the store, and also check the incoming name against stored aliases
        for alias in entity.aliases:
            alias_normalized = alias.lower().strip()
            existing = await self.find_by_alias_exact(alias_normalized)
            if existing is not None:
                raise EntityExistsError(
                    f"Entity alias '{alias}' already exists on entity '{existing.name}' (id={existing.id})"
                )

        # Also check whether the incoming name collides with any stored alias
        existing = await self.find_by_alias_exact(name_normalized)
        if existing is not None:
            raise EntityExistsError(
                f"Name '{entity.name}' collides with an alias on entity '{existing.name}' (id={existing.id})"
            )

        # Step 3: fuzzy similarity check
        warning: str | None = None
        similar = await self.find_similar_entities(entity.name, threshold=0.85)
        if similar:
            names = ", ".join(f"'{e.name}'" for e in similar)
            warning = (
                f"Entity '{entity.name}' is semantically similar to existing entities: {names}. "
                "Check for duplicates."
            )
            logger.warning(warning)

        # Step 4: prepare and store with embedding
        entity.prepare_for_save()
        vector = embed(entity.embedding_text)
        await self._store.upsert_entity(entity, vector)

        return entity, warning

    # ------------------------------------------------------------------
    # Lookup with multi-path resolution
    # ------------------------------------------------------------------

    async def resolve_entity(self, name: str) -> WorldEntity | list[WorldEntity] | None:
        """Resolve a name to entity(s).

        Resolution order:
        1. Exact name match (keyword index) → single result? return it
        2. Exact alias match (keyword index) → single result? return it; multiple? return list
        3. Semantic similarity (vector search, score > 0.5) →
           - single result with score > 0.8? return it (high confidence)
           - multiple results? return list (for disambiguation)
        4. No match → return None

        4 distinct return paths.
        """
        name_normalized = name.lower().strip()

        # Step 1: exact name match
        match = await self.find_by_name_exact(name_normalized)
        if match is not None:
            return match

        # Step 2: exact alias match
        alias_filter = models.Filter(
            must=[
                models.FieldCondition(
                    key="aliases_normalized",
                    match=models.MatchValue(value=name_normalized),
                ),
                models.FieldCondition(
                    key="status",
                    match=models.MatchValue(value="active"),
                ),
            ]
        )
        alias_matches = await self._store.scroll_entities(filter_conditions=alias_filter)
        if len(alias_matches) == 1:
            return alias_matches[0]
        if len(alias_matches) > 1:
            return alias_matches

        # Step 3: semantic similarity (score > 0.5)
        vector = embed(name)
        active_filter = models.Filter(
            must=[
                models.FieldCondition(
                    key="status",
                    match=models.MatchValue(value="active"),
                )
            ]
        )
        scored = await self._store.vector_search_entities(
            vector=vector,
            filter_conditions=active_filter,
            limit=5,
            score_threshold=0.5,
        )
        if not scored:
            return None

        if len(scored) == 1 and scored[0][1] > 0.8:
            return scored[0][0]

        # Multiple results — return list for disambiguation
        return [entity for entity, _score in scored]

    # ------------------------------------------------------------------
    # Helper methods
    # ------------------------------------------------------------------

    async def find_by_name_exact(self, name_normalized: str) -> WorldEntity | None:
        """Scroll with name_normalized filter + status=active."""
        scroll_filter = models.Filter(
            must=[
                models.FieldCondition(
                    key="name_normalized",
                    match=models.MatchValue(value=name_normalized),
                ),
                models.FieldCondition(
                    key="status",
                    match=models.MatchValue(value="active"),
                ),
            ]
        )
        results = await self._store.scroll_entities(filter_conditions=scroll_filter, limit=1)
        return results[0] if results else None

    async def find_by_alias_exact(self, alias_normalized: str) -> WorldEntity | None:
        """Scroll with aliases_normalized filter + status=active."""
        scroll_filter = models.Filter(
            must=[
                models.FieldCondition(
                    key="aliases_normalized",
                    match=models.MatchValue(value=alias_normalized),
                ),
                models.FieldCondition(
                    key="status",
                    match=models.MatchValue(value="active"),
                ),
            ]
        )
        results = await self._store.scroll_entities(filter_conditions=scroll_filter, limit=1)
        return results[0] if results else None

    async def find_similar_entities(
        self, name: str, threshold: float = 0.85
    ) -> list[WorldEntity]:
        """Vector search filtered to active entities, returning those above threshold."""
        vector = embed(name)
        active_filter = models.Filter(
            must=[
                models.FieldCondition(
                    key="status",
                    match=models.MatchValue(value="active"),
                )
            ]
        )
        scored = await self._store.vector_search_entities(
            vector=vector,
            filter_conditions=active_filter,
            limit=5,
            score_threshold=threshold,
        )
        return [entity for entity, _score in scored]
