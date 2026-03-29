"""
helix_memory.models.entity

Entity model: people, projects, concepts, locations, and other named things
the memory system tracks over time.
"""

from __future__ import annotations

from datetime import datetime
from typing import Literal, Optional
from uuid import uuid4

from pydantic import BaseModel, Field

from helix_memory.models.base import HelixBase, EMBEDDING_MODEL, EMBEDDING_VERSION

EntityType = Literal["project", "person", "client", "tool", "decision", "concept", "task", "payment"]
Scope = Literal["work", "personal", "global"]
Status = Literal["active", "paused", "completed", "archived"]
Priority = Literal["critical", "high", "medium", "low"]
RelationType = Literal[
    "works_on",
    "team_member",
    "client_of",
    "uses_tool",
    "decided_in",
    "blocked_by",
    "waiting_on",
    "reports_to",
    "related_to",
    "freelance_client",
    "depends_on",
    "contact_of",
    "belongs_to",
    "assigned_to",
]


class EntityRelation(BaseModel):
    target_id: str
    target_name: str
    type: RelationType
    detail: str = ""
    created_at: datetime = Field(default_factory=datetime.utcnow)


class WorldEntity(HelixBase):
    id: str = Field(default_factory=lambda: f"ent_{uuid4().hex}")
    type: EntityType
    name: str
    name_normalized: str = ""
    aliases: list[str] = []
    aliases_normalized: list[str] = []
    scope: Scope = "global"
    status: Status = "active"
    priority: Priority = "medium"
    context: str = ""
    tags: list[str] = []
    relations: list[EntityRelation] = []
    created_at: datetime = Field(default_factory=datetime.utcnow)
    updated_at: datetime = Field(default_factory=datetime.utcnow)
    last_interaction_at: datetime = Field(default_factory=datetime.utcnow)
    archived_at: Optional[datetime] = None
    embedding_text: str = ""
    embedding_model: str = EMBEDDING_MODEL
    embedding_version: str = EMBEDDING_VERSION

    def prepare_for_save(self) -> None:
        self.name_normalized = self.name.lower().strip()
        self.aliases_normalized = [a.lower().strip() for a in self.aliases]
        parts = [self.name] + self.aliases
        if self.context:
            parts.append(self.context)
        self.embedding_text = " ".join(parts)
        self.updated_at = datetime.utcnow()
