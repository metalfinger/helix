"""
helix_memory.models.interaction

Interaction model: individual conversation turns or events stored as episodic
memory. Includes importance scoring and decay metadata.
"""

from __future__ import annotations

from datetime import datetime, date
from typing import Literal, Optional
from uuid import uuid4

from pydantic import BaseModel, Field

from helix_memory.models.base import HelixBase, EMBEDDING_MODEL, EMBEDDING_VERSION

InteractionSource = Literal[
    "email",
    "mattermost",
    "telegram",
    "claude_code",
    "plane",
    "outline",
    "manual",
    "system",
    "meeting",
    "call",
    "in_person",
]

InteractionType = Literal[
    "message",
    "task_update",
    "decision",
    "meeting_note",
    "briefing",
    "research",
    "status_change",
    "reminder",
    "context_update",
]

MentionRole = Literal[
    "subject",
    "sender",
    "recipient",
    "assignee",
    "blocker",
    "mentioned",
    "decided_about",
]

ActionStatus = Literal["pending", "in_progress", "done", "cancelled", "stale"]


class EntityMention(BaseModel):
    entity_id: str
    entity_name: str
    role: MentionRole


class ActionItem(BaseModel):
    id: str = Field(default_factory=lambda: f"act_{uuid4().hex[:12]}")
    description: str
    assignee: Literal["self", "other"]
    assignee_name: str
    deadline: Optional[date] = None
    status: ActionStatus = "pending"
    linked_task_id: str = ""
    created_at: datetime = Field(default_factory=datetime.utcnow)
    resolved_at: Optional[datetime] = None


class Interaction(HelixBase):
    id: str = Field(default_factory=lambda: f"int_{uuid4().hex}")
    summary: str
    raw_ref: str = ""
    source: InteractionSource
    type: InteractionType
    entity_ids: list[str] = []
    entities: list[EntityMention] = []
    has_pending_actions: bool = False
    action_items: list[ActionItem] = []
    importance: float = 0.5
    timestamp: datetime = Field(default_factory=datetime.utcnow)
    created_at: datetime = Field(default_factory=datetime.utcnow)
    staleness_days: int = 30
    archived: bool = False
    archived_at: Optional[datetime] = None
    embedding_model: str = EMBEDDING_MODEL
    embedding_version: str = EMBEDDING_VERSION

    def prepare_for_save(self) -> None:
        self.entity_ids = [m.entity_id for m in self.entities]
        self.has_pending_actions = any(
            a.status in ("pending", "in_progress") for a in self.action_items
        )
