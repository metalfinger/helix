"""
helix_memory.models.base

Shared base model and common field types used across all memory models.
"""

from datetime import datetime, date
from typing import Optional
from uuid import uuid4
from pydantic import BaseModel, Field

SCHEMA_VERSION = 1
EMBEDDING_MODEL = "all-MiniLM-L6-v2"
EMBEDDING_VERSION = "1.0"
CONTEXT_MAX_WORDS = 500


class HelixBase(BaseModel):
    """Base model with schema versioning and forward compatibility."""
    schema_version: int = Field(default=SCHEMA_VERSION)

    model_config = {
        "extra": "ignore",
        "json_encoders": {
            datetime: lambda v: v.strftime("%Y-%m-%dT%H:%M:%SZ"),
            date: lambda v: v.isoformat(),
        }
    }
