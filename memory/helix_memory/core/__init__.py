"""Core modules — all business logic for helix-memory."""

from helix_memory.core.embeddings import embed, embed_batch, get_model_name
from helix_memory.core.file_writer import atomic_write
from helix_memory.core.store import HelixStore
from helix_memory.core.search import MemorySearch
from helix_memory.core.resolver import EntityResolver, EntityExistsError
from helix_memory.core.world_state_gen import WorldStateGenerator
from helix_memory.core.compactor import MemoryCompactor
from helix_memory.core.backup import BackupManager

__all__ = [
    "embed",
    "embed_batch",
    "get_model_name",
    "atomic_write",
    "HelixStore",
    "MemorySearch",
    "EntityResolver",
    "EntityExistsError",
    "WorldStateGenerator",
    "MemoryCompactor",
    "BackupManager",
]
