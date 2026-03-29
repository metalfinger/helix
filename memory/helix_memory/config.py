"""
Configuration loader for helix-memory.

Reads from ~/.helix/config.yaml if present, falls back to environment variables
prefixed with HELIX_. Uses pydantic-settings for validation and merging.
"""

import os
from pathlib import Path
from typing import Any, ClassVar, Dict, Tuple, Type

import yaml
from pydantic import BaseModel
from pydantic_settings import BaseSettings, PydanticBaseSettingsSource


_CONFIG_PATH = Path(os.path.expanduser("~/.helix/config.yaml"))


class QdrantConfig(BaseModel):
    url: str = "http://localhost:6333"
    api_key: str | None = None
    collection_prefix: str = "helix_"


class EmbeddingsConfig(BaseModel):
    model: str = "all-MiniLM-L6-v2"
    dimensions: int = 384
    device: str = "cpu"


class MemoryConfig(BaseModel):
    world_state_stale_hours: int = 6
    interaction_archive_days: int = 30
    interaction_prune_days: int = 90
    importance_decay_rate: float = 0.95
    stale_entity_days: int = 14
    context_max_words: int = 500
    max_search_results: int = 10


class WorldStateConfig(BaseModel):
    method: str = "programmatic"


class YamlConfigSource(PydanticBaseSettingsSource):
    """Loads settings from ~/.helix/config.yaml if it exists."""

    def get_field_value(self, field: Any, field_name: str) -> Tuple[Any, str, bool]:
        return None, field_name, False

    def __call__(self) -> Dict[str, Any]:
        if not _CONFIG_PATH.exists():
            return {}
        with _CONFIG_PATH.open("r", encoding="utf-8") as f:
            data = yaml.safe_load(f) or {}
        return data


class Settings(BaseSettings):
    qdrant: QdrantConfig = QdrantConfig()
    embeddings: EmbeddingsConfig = EmbeddingsConfig()
    memory: MemoryConfig = MemoryConfig()
    world_state: WorldStateConfig = WorldStateConfig()

    data_dir: Path = _CONFIG_PATH.parent

    model_config: ClassVar[dict] = {
        "env_prefix": "HELIX_",
        "env_nested_delimiter": "__",
    }

    @classmethod
    def settings_customise_sources(
        cls,
        settings_cls: Type[BaseSettings],
        init_settings: PydanticBaseSettingsSource,
        env_settings: PydanticBaseSettingsSource,
        dotenv_settings: PydanticBaseSettingsSource,
        **kwargs: Any,
    ) -> Tuple[PydanticBaseSettingsSource, ...]:
        return (
            init_settings,
            env_settings,
            YamlConfigSource(settings_cls),
        )


# Module-level singleton — import this everywhere
settings = Settings()
