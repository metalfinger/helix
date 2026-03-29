"""Initialize Qdrant collections and payload indexes for helix-memory.

Idempotent — safe to run multiple times. Skips existing collections.
"""

from __future__ import annotations

import asyncio
import logging

from qdrant_client import AsyncQdrantClient
from qdrant_client import models

from helix_memory.config import settings

logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
logger = logging.getLogger(__name__)

VECTOR_SIZE = 384
DISTANCE = models.Distance.COSINE


async def create_entities_collection(client: AsyncQdrantClient, name: str) -> None:
    """Create helix_entities collection if it does not already exist."""
    if await client.collection_exists(name):
        logger.info("Collection '%s' already exists — skipping.", name)
        return

    await client.create_collection(
        collection_name=name,
        vectors_config=models.VectorParams(
            size=VECTOR_SIZE,
            distance=DISTANCE,
            on_disk=False,
        ),
        optimizers_config=models.OptimizersConfigDiff(
            indexing_threshold=100,
        ),
    )
    logger.info("Created collection '%s'.", name)


async def create_interactions_collection(client: AsyncQdrantClient, name: str) -> None:
    """Create helix_interactions collection if it does not already exist."""
    if await client.collection_exists(name):
        logger.info("Collection '%s' already exists — skipping.", name)
        return

    await client.create_collection(
        collection_name=name,
        vectors_config=models.VectorParams(
            size=VECTOR_SIZE,
            distance=DISTANCE,
            on_disk=True,
        ),
        optimizers_config=models.OptimizersConfigDiff(
            indexing_threshold=500,
            memmap_threshold=1000,
        ),
    )
    logger.info("Created collection '%s'.", name)


async def create_entity_indexes(client: AsyncQdrantClient, name: str) -> None:
    """Create payload indexes on the entities collection."""
    keyword_fields = [
        "type",
        "scope",
        "status",
        "priority",
        "name_normalized",
        "aliases_normalized",
        "tags",
    ]

    for field in keyword_fields:
        await client.create_payload_index(
            collection_name=name,
            field_name=field,
            field_schema=models.KeywordIndexParams(
                type=models.KeywordIndexType.KEYWORD,
            ),
        )
        logger.info("  [%s] keyword index: %s", name, field)

    datetime_fields = ["updated_at", "last_interaction_at"]
    for field in datetime_fields:
        await client.create_payload_index(
            collection_name=name,
            field_name=field,
            field_schema=models.DatetimeIndexParams(
                type=models.DatetimeIndexType.DATETIME,
            ),
        )
        logger.info("  [%s] datetime index: %s", name, field)


async def create_interaction_indexes(client: AsyncQdrantClient, name: str) -> None:
    """Create payload indexes on the interactions collection."""
    keyword_fields = ["source", "type", "entity_ids"]
    for field in keyword_fields:
        await client.create_payload_index(
            collection_name=name,
            field_name=field,
            field_schema=models.KeywordIndexParams(
                type=models.KeywordIndexType.KEYWORD,
            ),
        )
        logger.info("  [%s] keyword index: %s", name, field)

    await client.create_payload_index(
        collection_name=name,
        field_name="timestamp",
        field_schema=models.DatetimeIndexParams(
            type=models.DatetimeIndexType.DATETIME,
        ),
    )
    logger.info("  [%s] datetime index: timestamp", name)

    await client.create_payload_index(
        collection_name=name,
        field_name="importance",
        field_schema=models.FloatIndexParams(
            type=models.FloatIndexType.FLOAT,
        ),
    )
    logger.info("  [%s] float index: importance", name)

    bool_fields = ["archived", "has_pending_actions"]
    for field in bool_fields:
        await client.create_payload_index(
            collection_name=name,
            field_name=field,
            field_schema=models.BoolIndexParams(
                type=models.BoolIndexType.BOOL,
            ),
        )
        logger.info("  [%s] bool index: %s", name, field)


async def main() -> None:
    prefix = settings.qdrant.collection_prefix
    entities_col = f"{prefix}entities"
    interactions_col = f"{prefix}interactions"

    logger.info("Connecting to Qdrant at %s", settings.qdrant.url)
    client = AsyncQdrantClient(url=settings.qdrant.url, api_key=settings.qdrant.api_key)

    try:
        await create_entities_collection(client, entities_col)
        await create_interactions_collection(client, interactions_col)

        logger.info("Creating entity indexes …")
        await create_entity_indexes(client, entities_col)

        logger.info("Creating interaction indexes …")
        await create_interaction_indexes(client, interactions_col)

        logger.info("Done. Collections and indexes are ready.")
    finally:
        await client.close()


if __name__ == "__main__":
    asyncio.run(main())
