"""
helix_memory.tools.write_tools

MCP tool handlers for writing to memory: helix_remember, helix_update_entity,
helix_create_entity, helix_log_interaction.
"""

from __future__ import annotations

import logging
from datetime import datetime, date
from typing import Any

from helix_memory.core.embeddings import embed
from helix_memory.core.resolver import EntityExistsError, EntityResolver
from helix_memory.models.entity import EntityRelation, WorldEntity
from helix_memory.models.interaction import ActionItem, EntityMention, Interaction
from helix_memory.utils.text_processing import truncate_to_words

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Type defaults
# ---------------------------------------------------------------------------

_IMPORTANCE_DEFAULTS: dict[str, float] = {
    "decision": 0.80,
    "status_update": 0.60,
    "note": 0.50,
    "context": 0.40,
    "action_item": 0.70,
    "preference": 0.50,
}

_TYPE_MAP: dict[str, str] = {
    "decision": "decision",
    "note": "message",
    "preference": "context_update",
    "action_item": "task_update",
    "context": "context_update",
    "status_update": "status_change",
}


def _validate_deadline(deadline: str) -> str | None:
    """Validate and normalize a deadline string. Returns ISO date or None if invalid."""
    if not deadline or deadline.lower() == "none":
        return None
    # Try ISO date directly
    try:
        date.fromisoformat(deadline)
        return deadline
    except ValueError:
        pass
    return None  # Invalid — caller should return an error


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------

async def _resolve_entity_names(
    resolver: EntityResolver,
    names: list[str],
) -> tuple[list[WorldEntity], list[str]]:
    """Resolve a list of entity names. Returns (resolved_entities, warnings)."""
    resolved: list[WorldEntity] = []
    warnings: list[str] = []

    for name in names:
        result = await resolver.resolve_entity(name)
        if result is None:
            warnings.append(f"Entity '{name}' not found — skipping link.")
        elif isinstance(result, list):
            # Ambiguous — pick the first but warn
            warnings.append(
                f"Entity '{name}' is ambiguous ({len(result)} matches: "
                + ", ".join(f"'{e.name}'" for e in result)
                + "). Using first match."
            )
            resolved.append(result[0])
        else:
            resolved.append(result)

    return resolved, warnings


async def _update_entity_timestamp(
    store: Any,
    entity: WorldEntity,
) -> None:
    """Stamp last_interaction_at = now, re-embed, upsert."""
    entity.last_interaction_at = datetime.utcnow()
    entity.prepare_for_save()
    vector = embed(entity.embedding_text)
    await store.upsert_entity(entity, vector)


# ---------------------------------------------------------------------------
# Tool 1: helix_remember
# ---------------------------------------------------------------------------

async def helix_remember(
    ctx: dict,
    content: str,
    type: str = "note",
    related_to: list[str] | None = None,
    importance: float | None = None,
) -> dict:
    """Store something important. Resolves entity names, links relations, stores as interaction.

    Args:
        ctx: Runtime context with store, search, resolver, world_state_gen.
        content: The content to remember.
        type: One of decision | note | preference | action_item | context | status_update.
        related_to: Optional list of entity names to link.
        importance: Override importance 0.0-1.0; defaults by type if not provided.

    Returns:
        {interaction_id, entities_linked, warning}
    """
    store = ctx["store"]
    resolver = ctx["resolver"]
    world_state_gen = ctx.get("world_state_gen")

    # Step 1: resolve related entity names
    entities_resolved: list[WorldEntity] = []
    all_warnings: list[str] = []

    if related_to:
        entities_resolved, resolve_warnings = await _resolve_entity_names(resolver, related_to)
        all_warnings.extend(resolve_warnings)

    # Step 2: build EntityMention objects
    mentions = [
        EntityMention(
            entity_id=entity.id,
            entity_name=entity.name,
            role="subject",
        )
        for entity in entities_resolved
    ]

    # Special handling: decisions should be stored as entities, not just interactions
    if type == "decision":
        # Create a decision entity in addition to the interaction
        try:
            decision_entity = WorldEntity(
                type="decision",
                name=content[:80],  # First 80 chars as name
                context=content,
                scope=entities_resolved[0].scope if entities_resolved else "global",
            )
            # Add relations to resolved entities
            for entity in entities_resolved:
                decision_entity.relations.append(EntityRelation(
                    target_id=entity.id,
                    target_name=entity.name,
                    type="related_to",
                ))
            decision_entity.prepare_for_save()
            dec_vector = embed(decision_entity.embedding_text)
            await store.upsert_entity(decision_entity, dec_vector)
        except Exception:
            logger.warning("Failed to create decision entity from helix_remember", exc_info=True)

    # Step 3: determine importance
    effective_importance = importance if importance is not None else _IMPORTANCE_DEFAULTS.get(type, 0.50)

    # Step 4: map input type to InteractionType
    interaction_type = _TYPE_MAP.get(type, "message")

    # Step 5: create Interaction
    interaction = Interaction(
        summary=content,
        source="claude_code",
        type=interaction_type,  # type: ignore[arg-type]
        entities=mentions,
        importance=effective_importance,
    )

    # Step 6: prepare_for_save
    interaction.prepare_for_save()

    # Step 7: embed and store
    vector = embed(interaction.summary)
    await store.upsert_interaction(interaction, vector)

    # Step 8: update entity timestamps
    for entity in entities_resolved:
        await _update_entity_timestamp(store, entity)

    # Step 9: flag world state dirty if importance > 0.7
    if effective_importance > 0.7 and world_state_gen is not None:
        try:
            await world_state_gen.generate(force=True)
        except Exception:
            logger.warning("World state regeneration failed after helix_remember", exc_info=True)

    warning = "; ".join(all_warnings) if all_warnings else None

    return {
        "interaction_id": interaction.id,
        "entities_linked": [e.name for e in entities_resolved],
        "warning": warning,
    }


# ---------------------------------------------------------------------------
# Tool 2: helix_update_entity
# ---------------------------------------------------------------------------

async def helix_update_entity(
    ctx: dict,
    name: str,
    updates: dict,
) -> dict:
    """Update a project, person, or concept. Context is rewritten, not appended.

    Args:
        ctx: Runtime context.
        name: Entity name (fuzzy matched).
        updates: Dict of fields to update. Supported: status, priority, context, tags, aliases, etc.

    Returns:
        Updated entity as dict, or disambiguation list if ambiguous.
    """
    store = ctx["store"]
    resolver = ctx["resolver"]

    # Step 1 & 2: resolve entity
    result = await resolver.resolve_entity(name)
    if result is None:
        return {"error": f"Entity '{name}' not found."}

    if isinstance(result, list):
        return {
            "error": "ambiguous",
            "candidates": [{"id": e.id, "name": e.name, "type": e.type} for e in result],
            "message": f"'{name}' matched {len(result)} entities. Provide a more specific name.",
        }

    entity = result

    # Step 3: apply updates
    context_changed = False
    allowed_fields = {
        "status", "priority", "context", "tags", "aliases", "scope",
        "name", "type",
    }

    for field, value in updates.items():
        if field not in allowed_fields:
            logger.warning("helix_update_entity: ignoring unknown field '%s'", field)
            continue
        if field == "context":
            context_changed = True
        setattr(entity, field, value)

    # Step 4: truncate context if changed
    if context_changed and entity.context:
        entity.context = truncate_to_words(entity.context, max_words=500)

    # Step 5: prepare_for_save
    entity.prepare_for_save()

    # Step 6: re-embed (embedding_text rebuilt in prepare_for_save)
    vector = embed(entity.embedding_text)

    # Step 7: upsert
    await store.upsert_entity(entity, vector)

    # Step 8: log a context_update interaction if context changed
    if context_changed:
        log_interaction = Interaction(
            summary=f"Context updated for entity '{entity.name}'.",
            source="claude_code",
            type="context_update",
            entities=[EntityMention(entity_id=entity.id, entity_name=entity.name, role="subject")],
            importance=0.4,
        )
        log_interaction.prepare_for_save()
        log_vector = embed(log_interaction.summary)
        await store.upsert_interaction(log_interaction, log_vector)

    # Refresh world state so TUI updates live
    world_state_gen = ctx.get("world_state_gen")
    if world_state_gen:
        try:
            await world_state_gen.generate(force=True)
        except Exception:
            logger.warning("World state regen failed after entity update", exc_info=True)

    return entity.model_dump(mode="json")


# ---------------------------------------------------------------------------
# Tool 3: helix_create_entity
# ---------------------------------------------------------------------------

async def helix_create_entity(
    ctx: dict,
    type: str,
    name: str,
    aliases: list[str] | None = None,
    scope: str = "global",
    context: str = "",
    priority: str = "medium",
    relations: list[dict] | None = None,
    tags: list[str] | None = None,
) -> dict:
    """Create a new entity with full control over fields. Runs dedupe check.

    Args:
        ctx: Runtime context.
        type: EntityType — project | person | client | tool | decision | concept | task.
        name: Entity name.
        aliases: Optional list of alternate names.
        scope: One of work | personal | global.
        context: Descriptive context string (truncated to 500 words).
        priority: critical | high | medium | low.
        relations: Optional list of {target_name, type, detail} dicts.
        tags: Optional list of tags.

    Returns:
        {entity: dict, warning: str|None}
    """
    store = ctx["store"]
    resolver = ctx["resolver"]

    # Step 1 & 2: build entity and truncate context
    truncated_context = truncate_to_words(context, max_words=500) if context else ""

    entity = WorldEntity(
        type=type,  # type: ignore[arg-type]
        name=name,
        aliases=aliases or [],
        scope=scope,  # type: ignore[arg-type]
        context=truncated_context,
        priority=priority,  # type: ignore[arg-type]
        tags=tags or [],
    )

    # Step 3: create via resolver (handles dedupe + prepare_for_save + embed + upsert)
    warning: str | None = None
    try:
        entity, warning = await resolver.create_entity(entity)
    except EntityExistsError as exc:
        return {"error": str(exc)}

    # Step 4 & 5: resolve and attach relations
    if relations:
        for rel_dict in relations:
            target_name = rel_dict.get("target_name", "")
            rel_type = rel_dict.get("type", "related_to")
            detail = rel_dict.get("detail", "")

            target_id = ""
            if target_name:
                target_result = await resolver.resolve_entity(target_name)
                if isinstance(target_result, WorldEntity):
                    target_id = target_result.id
                    target_name = target_result.name  # normalise to canonical name
                elif isinstance(target_result, list) and target_result:
                    # Use first match
                    target_id = target_result[0].id
                    target_name = target_result[0].name

            entity.relations.append(
                EntityRelation(
                    target_id=target_id,
                    target_name=target_name,
                    type=rel_type,  # type: ignore[arg-type]
                    detail=detail,
                )
            )

        # Re-save with relations attached
        entity.prepare_for_save()
        vector = embed(entity.embedding_text)
        await store.upsert_entity(entity, vector)

    # Refresh world state so TUI updates live
    world_state_gen = ctx.get("world_state_gen")
    if world_state_gen:
        try:
            await world_state_gen.generate(force=True)
        except Exception:
            logger.warning("World state regen failed after entity creation", exc_info=True)

    return {
        "entity": entity.model_dump(mode="json"),
        "warning": warning,
    }


# ---------------------------------------------------------------------------
# Tool 4: helix_log_interaction
# ---------------------------------------------------------------------------

async def helix_log_interaction(
    ctx: dict,
    summary: str,
    source: str = "manual",
    entities: list[str] | None = None,
    action_items: list[dict] | None = None,
    importance: float = 0.6,
) -> dict:
    """Log an event — meeting, call, conversation.

    Args:
        ctx: Runtime context.
        summary: What happened.
        source: meeting | call | in_person | manual.
        entities: Optional entity names to link.
        action_items: Optional list of {description, assignee, deadline} dicts.
        importance: 0.0-1.0 (default 0.6).

    Returns:
        {interaction_id, entities_linked, action_items_created}
    """
    store = ctx["store"]
    resolver = ctx["resolver"]

    # Step 1: resolve entity names
    entities_resolved: list[WorldEntity] = []
    if entities:
        entities_resolved, _ = await _resolve_entity_names(resolver, entities)

    # Step 2: build EntityMention objects (role="mentioned")
    mentions = [
        EntityMention(
            entity_id=entity.id,
            entity_name=entity.name,
            role="mentioned",
        )
        for entity in entities_resolved
    ]

    # Step 3: build ActionItem objects
    built_action_items: list[ActionItem] = []
    if action_items:
        for item_dict in action_items:
            description = item_dict.get("description", "")
            if not description:
                continue

            assignee_raw = item_dict.get("assignee", "self")
            # Normalise to Literal["self", "other"]
            assignee: str = "self" if assignee_raw in ("self", "me", "I") else "other"
            assignee_name: str = item_dict.get("assignee_name", assignee_raw)

            deadline_raw = item_dict.get("deadline")
            deadline: date | None = None
            if deadline_raw:
                if isinstance(deadline_raw, date):
                    deadline = deadline_raw
                elif isinstance(deadline_raw, str):
                    try:
                        deadline = date.fromisoformat(deadline_raw)
                    except ValueError:
                        logger.warning("Could not parse deadline '%s' — ignoring.", deadline_raw)

            built_action_items.append(
                ActionItem(
                    description=description,
                    assignee=assignee,  # type: ignore[arg-type]
                    assignee_name=assignee_name,
                    deadline=deadline,
                )
            )

    # Step 4: create Interaction
    interaction = Interaction(
        summary=summary,
        source=source,  # type: ignore[arg-type]
        type="meeting_note",
        entities=mentions,
        action_items=built_action_items,
        importance=importance,
    )

    # Step 5: prepare_for_save
    interaction.prepare_for_save()

    # Step 6: embed
    vector = embed(summary)

    # Step 7: store
    await store.upsert_interaction(interaction, vector)

    # Step 8: update entity timestamps
    for entity in entities_resolved:
        await _update_entity_timestamp(store, entity)

    # Refresh world state so TUI updates live
    world_state_gen = ctx.get("world_state_gen")
    if world_state_gen:
        try:
            await world_state_gen.generate(force=True)
        except Exception:
            logger.warning("World state regen failed after interaction logging", exc_info=True)

    return {
        "interaction_id": interaction.id,
        "entities_linked": [e.name for e in entities_resolved],
        "action_items_created": [ai.id for ai in built_action_items],
    }


# ---------------------------------------------------------------------------
# Tool 5: helix_create_task
# ---------------------------------------------------------------------------

async def helix_create_task(
    ctx: dict,
    title: str,
    project: str | None = None,
    description: str = "",
    priority: str = "medium",
    deadline: str | None = None,
    scope: str = "global",
    linked_entities: list[str] | None = None,
) -> dict:
    """Create a task and link it to a project and/or other entities.

    Convenience wrapper around entity creation — creates an entity of type "task"
    with automatic belongs_to relation to the project and related_to for other entities.

    Args:
        ctx: Runtime context.
        title: Task title (becomes entity name).
        project: Optional project name to link via belongs_to relation.
        description: Task description (becomes entity context).
        priority: critical | high | medium | low.
        deadline: Optional ISO date string (YYYY-MM-DD) — stored as deadline-* tag.
        scope: work | personal | global.
        linked_entities: Optional list of other entity names to link (decisions, people, etc.).

    Returns:
        {entity: dict, warning: str|None}
    """
    resolver = ctx["resolver"]
    warnings: list[str] = []

    # Validate project exists — warn loudly if not
    if project:
        project_result = await resolver.resolve_entity(project)
        if project_result is None:
            warnings.append(
                f"WARNING: Project '{project}' does not exist in memory. "
                f"Task created but NOT linked to any project. "
                f"Create the project first with helix_create_entity, then retry."
            )
            project = None  # Don't create a dangling belongs_to
        elif isinstance(project_result, list):
            warnings.append(
                f"Project '{project}' is ambiguous ({len(project_result)} matches: "
                + ", ".join(f"'{e.name}'" for e in project_result)
                + "). Using first match."
            )
            # Inherit scope from project if caller used default
            if scope == "global":
                scope = project_result[0].scope
            project = project_result[0].name
        else:
            # Single match — inherit scope from project if caller used default
            if scope == "global":
                scope = project_result.scope

    relations: list[dict] = []
    if project:
        relations.append({"target_name": project, "type": "belongs_to", "detail": ""})
    if linked_entities:
        for name in linked_entities:
            relations.append({"target_name": name, "type": "related_to", "detail": ""})

    tags: list[str] = ["task"]
    if deadline:
        validated = _validate_deadline(deadline)
        if validated is None:
            return {"error": f"Invalid deadline '{deadline}'. Use ISO format (YYYY-MM-DD)."}
        tags.append(f"deadline-{validated}")

    result = await helix_create_entity(
        ctx,
        type="task",
        name=title,
        scope=scope,
        context=description,
        priority=priority,
        relations=relations if relations else None,
        tags=tags,
    )

    # Merge warnings
    if warnings:
        existing_warning = result.get("warning") or ""
        result["warning"] = "; ".join(filter(None, [existing_warning] + warnings))

    # Always refresh world state so helix TUI updates live
    world_state_gen = ctx.get("world_state_gen")
    if world_state_gen:
        try:
            await world_state_gen.generate(force=True)
        except Exception:
            logger.warning("World state regen failed after task creation", exc_info=True)

    return result


# ---------------------------------------------------------------------------
# Tool 6: helix_complete_task
# ---------------------------------------------------------------------------

async def helix_complete_task(
    ctx: dict,
    name: str,
    note: str = "",
) -> dict:
    """Mark a task as completed.

    Args:
        ctx: Runtime context.
        name: Task name (fuzzy matched).
        note: Optional completion note appended to context.

    Returns:
        Updated entity dict or error.
    """
    store = ctx["store"]
    resolver = ctx["resolver"]
    world_state_gen = ctx.get("world_state_gen")

    result = await resolver.resolve_entity(name)
    if result is None:
        return {"error": f"Task '{name}' not found."}
    if isinstance(result, list):
        return {
            "error": "ambiguous",
            "candidates": [{"id": e.id, "name": e.name, "type": e.type} for e in result],
            "message": f"'{name}' matched {len(result)} entities. Be more specific.",
        }

    entity = result
    if entity.type != "task":
        return {"error": f"'{entity.name}' is a {entity.type}, not a task."}

    entity.status = "completed"
    if note:
        entity.context = f"{entity.context}\n\nCompleted: {note}".strip()
        entity.context = truncate_to_words(entity.context, max_words=500)

    entity.prepare_for_save()
    vector = embed(entity.embedding_text)
    await store.upsert_entity(entity, vector)

    # Log completion as interaction
    mentions = [EntityMention(entity_id=entity.id, entity_name=entity.name, role="subject")]
    for rel in entity.relations:
        if rel.type == "belongs_to" and rel.target_id:
            mentions.append(EntityMention(
                entity_id=rel.target_id,
                entity_name=rel.target_name,
                role="mentioned",
            ))

    interaction = Interaction(
        summary=f"Task completed: {entity.name}" + (f" — {note}" if note else ""),
        source="claude_code",
        type="task_update",
        entities=mentions,
        importance=0.6,
    )
    interaction.prepare_for_save()
    log_vector = embed(interaction.summary)
    await store.upsert_interaction(interaction, log_vector)

    # Update linked project's last_interaction_at so it doesn't go stale
    for rel in entity.relations:
        if rel.type == "belongs_to" and rel.target_id:
            proj_result = await resolver.resolve_entity(rel.target_name)
            if isinstance(proj_result, WorldEntity):
                await _update_entity_timestamp(store, proj_result)

    if world_state_gen:
        try:
            await world_state_gen.generate(force=True)
        except Exception:
            logger.warning("World state regen failed after task completion", exc_info=True)

    return {"entity": entity.model_dump(mode="json"), "completed": True}


# ---------------------------------------------------------------------------
# Tool 7: helix_update_task
# ---------------------------------------------------------------------------

async def helix_update_task(
    ctx: dict,
    name: str,
    priority: str | None = None,
    deadline: str | None = None,
    description: str | None = None,
    status: str | None = None,
    project: str | None = None,
    add_linked: list[str] | None = None,
) -> dict:
    """Update a task's properties.

    Args:
        ctx: Runtime context.
        name: Task name (fuzzy matched).
        priority: New priority (critical/high/medium/low).
        deadline: New deadline (ISO date YYYY-MM-DD, or "none" to remove).
        description: New description (replaces context).
        status: New status (active/paused/completed/archived).
        project: New project to link to (replaces existing belongs_to).
        add_linked: Additional entities to link via related_to.

    Returns:
        Updated entity dict or error.
    """
    store = ctx["store"]
    resolver = ctx["resolver"]
    world_state_gen = ctx.get("world_state_gen")

    result = await resolver.resolve_entity(name)
    if result is None:
        return {"error": f"Task '{name}' not found."}
    if isinstance(result, list):
        return {
            "error": "ambiguous",
            "candidates": [{"id": e.id, "name": e.name, "type": e.type} for e in result],
            "message": f"'{name}' matched {len(result)} entities. Be more specific.",
        }

    entity = result
    if entity.type != "task":
        return {"error": f"'{entity.name}' is a {entity.type}, not a task."}

    # Apply updates
    if priority:
        entity.priority = priority
    if description is not None:
        entity.context = truncate_to_words(description, max_words=500)
    if status:
        entity.status = status

    # Handle deadline tag
    if deadline is not None:
        # Remove existing deadline tags
        entity.tags = [t for t in entity.tags if not t.startswith("deadline-")]
        if deadline.lower() != "none" and deadline:
            validated = _validate_deadline(deadline)
            if validated is None:
                return {"error": f"Invalid deadline '{deadline}'. Use ISO format (YYYY-MM-DD)."}
            entity.tags.append(f"deadline-{validated}")

    # Handle project re-linking
    if project is not None:
        # Remove existing belongs_to relations
        entity.relations = [r for r in entity.relations if r.type != "belongs_to"]
        if project:
            proj_result = await resolver.resolve_entity(project)
            if proj_result is None:
                return {"error": f"Project '{project}' not found. Task not updated."}
            if isinstance(proj_result, list):
                proj_result = proj_result[0]
            entity.relations.append(EntityRelation(
                target_id=proj_result.id,
                target_name=proj_result.name,
                type="belongs_to",
            ))

    # Handle additional links
    if add_linked:
        for link_name in add_linked:
            link_result = await resolver.resolve_entity(link_name)
            if isinstance(link_result, WorldEntity):
                entity.relations.append(EntityRelation(
                    target_id=link_result.id,
                    target_name=link_result.name,
                    type="related_to",
                ))

    entity.prepare_for_save()
    vector = embed(entity.embedding_text)
    await store.upsert_entity(entity, vector)

    # Refresh world state
    if world_state_gen:
        try:
            await world_state_gen.generate(force=True)
        except Exception:
            logger.warning("World state regen failed after task update", exc_info=True)

    return {"entity": entity.model_dump(mode="json"), "updated": True}


# ---------------------------------------------------------------------------
# Tool 8: helix_delete_entity
# ---------------------------------------------------------------------------

async def helix_delete_entity(
    ctx: dict,
    name: str,
    confirm: bool = False,
) -> dict:
    """Permanently delete an entity from memory. DESTRUCTIVE — cannot be undone.

    Args:
        ctx: Runtime context.
        name: Entity name (fuzzy matched).
        confirm: Must be True to actually delete. If False, returns entity info for confirmation.

    Returns:
        Deleted entity info or confirmation prompt.
    """
    store = ctx["store"]
    resolver = ctx["resolver"]
    world_state_gen = ctx.get("world_state_gen")

    result = await resolver.resolve_entity(name)
    if result is None:
        return {"error": f"Entity '{name}' not found."}
    if isinstance(result, list):
        return {
            "error": "ambiguous",
            "candidates": [{"id": e.id, "name": e.name, "type": e.type} for e in result],
            "message": f"'{name}' matched {len(result)} entities. Be more specific.",
        }

    entity = result

    if not confirm:
        return {
            "confirm_required": True,
            "entity": {"id": entity.id, "name": entity.name, "type": entity.type, "status": entity.status},
            "message": f"About to permanently delete '{entity.name}' ({entity.type}). Call again with confirm=True to proceed.",
        }

    await store.delete_entity(entity.id)

    # Refresh world state
    if world_state_gen:
        try:
            await world_state_gen.generate(force=True)
        except Exception:
            logger.warning("World state regen failed after entity deletion", exc_info=True)

    return {"deleted": True, "name": entity.name, "type": entity.type, "id": entity.id}
