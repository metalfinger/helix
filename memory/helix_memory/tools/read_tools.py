"""
helix_memory.tools.read_tools

MCP tool handlers for reading from memory: search_memory, get_entity,
get_world_state, list_entities, and related query operations.
Populated in Epic 5a (E5a.1).
"""

from __future__ import annotations

from typing import Optional

from helix_memory.models.entity import WorldEntity


# ---------------------------------------------------------------------------
# Tool 1: helix_get_world_state
# ---------------------------------------------------------------------------

async def helix_get_world_state(ctx: dict) -> str:
    """Get current situation — active projects, urgent items, deadlines, blockers.

    CALL THIS FIRST in every new session.
    Returns markdown, 500-1500 tokens. Auto-regenerates if stale.
    """
    ws = await ctx["world_state_gen"].generate()
    return ws.document


# ---------------------------------------------------------------------------
# Tool 2: helix_search_memory
# ---------------------------------------------------------------------------

async def helix_search_memory(
    ctx: dict,
    query: str,
    scope: Optional[str] = None,
    types: Optional[list[str]] = None,
    time_range: Optional[str] = None,
    limit: int = 5,
) -> list[dict]:
    """Semantic search across all knowledge.

    Returns list of {type, name/summary, score, details}.

    Args:
        ctx:        Tool context with store/search/resolver/world_state_gen instances.
        query:      Free-text search query.
        scope:      Optional scope filter — "work", "personal", or "global".
        types:      Optional entity type filter — e.g. ["project", "person"].
        time_range: Optional interaction time filter — "today", "this_week", or "this_month".
        limit:      Maximum number of results to return (default 5).
    """
    results = await ctx["search"].semantic_search(
        query=query,
        scope=scope,
        entity_types=types,
        time_range=time_range,
        limit=limit,
    )

    formatted: list[dict] = []
    for r in results:
        if r["type"] == "entity":
            entity = r["data"]
            formatted.append({
                "type": "entity",
                "name": entity.name,
                "entity_type": entity.type,
                "score": round(r["score"], 3),
                "context": entity.context[:200],
                "status": entity.status,
            })
        else:
            interaction = r["data"]
            formatted.append({
                "type": "interaction",
                "summary": interaction.summary,
                "source": interaction.source,
                "score": round(r["score"], 3),
                "timestamp": interaction.timestamp.strftime("%Y-%m-%dT%H:%M:%SZ"),
            })

    return formatted


# ---------------------------------------------------------------------------
# Tool 3: helix_get_entity
# ---------------------------------------------------------------------------

async def helix_get_entity(ctx: dict, name: str) -> dict:
    """Full details about a project, person, client, or concept.

    Includes context, relations, and last 5 interactions.
    Fuzzy matches name via resolver.

    Returns dict with entity details + relations + recent interactions.
    If multiple matches, returns disambiguation list.

    Args:
        ctx:  Tool context with store/search/resolver/world_state_gen instances.
        name: Entity name (fuzzy matched via resolver).
    """
    result = await ctx["resolver"].resolve_entity(name)

    # No match
    if result is None:
        return {"error": f"No entity found matching '{name}'"}

    # Multiple matches — return disambiguation list
    if isinstance(result, list):
        return {
            "disambiguation": True,
            "query": name,
            "matches": [
                {
                    "name": e.name,
                    "type": e.type,
                    "scope": e.scope,
                    "status": e.status,
                    "context": e.context[:120],
                }
                for e in result
            ],
        }

    # Single entity — fetch timeline and format full details
    entity: WorldEntity = result

    recent_interactions_raw = await ctx["search"].get_entity_timeline(
        entity_id=entity.id,
        days=30,
    )
    # Cap to last 5
    recent_interactions_raw = recent_interactions_raw[:5]

    recent_interactions = [
        {
            "summary": i.summary,
            "source": i.source,
            "type": i.type,
            "timestamp": i.timestamp.strftime("%Y-%m-%dT%H:%M:%SZ"),
        }
        for i in recent_interactions_raw
    ]

    relations = [
        {
            "type": r.type,
            "target_name": r.target_name,
            "detail": r.detail,
        }
        for r in entity.relations
    ]

    return {
        "id": entity.id,
        "name": entity.name,
        "type": entity.type,
        "scope": entity.scope,
        "status": entity.status,
        "priority": entity.priority,
        "context": entity.context,
        "tags": entity.tags,
        "aliases": entity.aliases,
        "relations": relations,
        "created_at": entity.created_at.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "updated_at": entity.updated_at.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "last_interaction_at": entity.last_interaction_at.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "recent_interactions": recent_interactions,
    }


# ---------------------------------------------------------------------------
# Tool 4: helix_get_timeline
# ---------------------------------------------------------------------------

async def helix_get_timeline(
    ctx: dict,
    entity_name: str,
    days: int = 14,
) -> list[dict]:
    """Chronological interaction history for an entity.

    Resolve name first, then get timeline.
    Returns list of interaction summaries, newest first.

    Args:
        ctx:         Tool context with store/search/resolver/world_state_gen instances.
        entity_name: Entity name (fuzzy matched via resolver).
        days:        How many days back to look (default 14).
    """
    result = await ctx["resolver"].resolve_entity(entity_name)

    if result is None:
        return [{"error": f"No entity found matching '{entity_name}'"}]

    # If multiple matches, use the first one and note the ambiguity
    if isinstance(result, list):
        entity: WorldEntity = result[0]
        ambiguous = True
    else:
        entity = result
        ambiguous = False

    interactions = await ctx["search"].get_entity_timeline(
        entity_id=entity.id,
        days=days,
    )

    timeline = []

    if ambiguous:
        timeline.append({
            "note": f"Ambiguous name '{entity_name}' — showing timeline for '{entity.name}'. "
                    "Use helix_get_entity for a disambiguation list.",
        })

    for i in interactions:
        entry: dict = {
            "summary": i.summary,
            "source": i.source,
            "type": i.type,
            "timestamp": i.timestamp.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "importance": i.importance,
        }
        if i.action_items:
            entry["action_items"] = [
                {
                    "description": a.description,
                    "status": a.status,
                    "assignee": a.assignee_name,
                    "deadline": a.deadline.isoformat() if a.deadline else None,
                }
                for a in i.action_items
            ]
        timeline.append(entry)

    return timeline


# ---------------------------------------------------------------------------
# Tool 5: helix_list_entities
# ---------------------------------------------------------------------------

async def helix_list_entities(
    ctx: dict,
    type: Optional[str] = None,
    scope: Optional[str] = None,
    status: str = "active",
) -> list[dict]:
    """List all entities, optionally filtered.

    Returns list of {name, type, scope, status, last_interaction}.

    Args:
        ctx:    Tool context with store/search/resolver/world_state_gen instances.
        type:   Optional entity type filter — e.g. "project", "person", "client".
        scope:  Optional scope filter — "work", "personal", or "global".
        status: Status filter (default "active"). Use "all" to skip status filtering.
    """
    from qdrant_client import models as qdrant_models

    conditions = []

    if status and status != "all":
        conditions.append(
            qdrant_models.FieldCondition(
                key="status",
                match=qdrant_models.MatchValue(value=status),
            )
        )

    if scope:
        conditions.append(
            qdrant_models.FieldCondition(
                key="scope",
                match=qdrant_models.MatchValue(value=scope),
            )
        )

    if type:
        conditions.append(
            qdrant_models.FieldCondition(
                key="type",
                match=qdrant_models.MatchValue(value=type),
            )
        )

    filter_conditions = qdrant_models.Filter(must=conditions) if conditions else None

    entities = await ctx["store"].scroll_entities(
        filter_conditions=filter_conditions,
        limit=500,
    )

    return [
        {
            "name": e.name,
            "type": e.type,
            "scope": e.scope,
            "status": e.status,
            "priority": e.priority,
            "last_interaction": e.last_interaction_at.strftime("%Y-%m-%dT%H:%M:%SZ"),
        }
        for e in entities
    ]


# ---------------------------------------------------------------------------
# Tool 7: helix_list_tasks
# ---------------------------------------------------------------------------

async def helix_list_tasks(
    ctx: dict,
    project: Optional[str] = None,
    status: str = "active",
    scope: Optional[str] = None,
) -> list[dict]:
    """List tasks, optionally filtered by project, status, or scope.

    Args:
        ctx: Tool context.
        project: Optional project name — filters to tasks with belongs_to relation to this project.
        status: "active" (default), "completed", "all".
        scope: Optional scope filter.

    Returns:
        List of task dicts with name, priority, status, deadline, project, linked entities.
    """
    from qdrant_client import models as qdrant_models

    conditions = [
        qdrant_models.FieldCondition(
            key="type",
            match=qdrant_models.MatchValue(value="task"),
        )
    ]

    if status and status != "all":
        conditions.append(
            qdrant_models.FieldCondition(
                key="status",
                match=qdrant_models.MatchValue(value=status),
            )
        )

    if scope:
        conditions.append(
            qdrant_models.FieldCondition(
                key="scope",
                match=qdrant_models.MatchValue(value=scope),
            )
        )

    filter_conditions = qdrant_models.Filter(must=conditions)
    entities = await ctx["store"].scroll_entities(
        filter_conditions=filter_conditions,
        limit=500,
    )

    if project:
        resolver = ctx["resolver"]
        project_entity = await resolver.resolve_entity(project)
        if project_entity and not isinstance(project_entity, list):
            entities = [
                e for e in entities
                if any(r.target_id == project_entity.id and r.type == "belongs_to" for r in e.relations)
            ]

    results = []
    for e in entities:
        deadline = None
        for tag in e.tags:
            if tag.startswith("deadline-"):
                deadline = tag[len("deadline-"):]
                break

        proj_name = ""
        linked = []
        for rel in e.relations:
            if rel.type == "belongs_to":
                proj_name = rel.target_name
            elif rel.type in ("related_to", "decided_in", "assigned_to"):
                linked.append(rel.target_name)

        results.append({
            "name": e.name,
            "status": e.status,
            "priority": e.priority,
            "scope": e.scope,
            "context": e.context[:120],
            "deadline": deadline,
            "project": proj_name,
            "linked_entities": linked,
            "created_at": e.created_at.strftime("%Y-%m-%dT%H:%M:%SZ"),
        })

    # Sort: priority (critical > high > medium > low), then newest first
    _priority_order = {"critical": 0, "high": 1, "medium": 2, "low": 3}
    results.sort(key=lambda t: (_priority_order.get(t["priority"], 99), t["created_at"]))

    return results


# ---------------------------------------------------------------------------
# Tool 6: helix_get_instructions
# ---------------------------------------------------------------------------

async def helix_get_instructions(ctx: dict) -> str:
    """Returns instructions for how to use helix-memory tools effectively.

    Any LLM connecting for the first time should call this.
    """
    return """# Helix Memory Integration

You have access to helix-memory, a persistent memory and task system, via MCP.
ALWAYS call `helix_get_world_state` at the START of every session.

This gives you awareness of:
- Active projects (work and personal/freelance)
- Urgent items and upcoming deadlines
- What the user is waiting on from others
- Pending tasks and action items
- Stale threads that need attention
- Recent decisions for reference

## Core tools:
- Start of session → `helix_get_world_state`
- Need detail on a topic → `helix_search_memory` or `helix_get_entity`
- Important decision made → `helix_remember` with type "decision"
- Status update → `helix_remember` with type "status_update"
- After a meeting/call → `helix_log_interaction`
- Asking about a person/project → `helix_get_entity`
- "What happened with X?" → `helix_get_timeline`

## Task tools:
- "Add task" / "I need to..." / "todo" → `helix_create_task` (link to project!)
- "Done with X" / "Finished X" → `helix_complete_task`
- "What are my tasks?" / "Show tasks" → `helix_list_tasks`
- When action items emerge in conversation → offer to create as tasks

## Daily companion behavior:
- When user says "what should I do today?" or "daily briefing":
  1. Get world state for current context
  2. List active tasks across all projects
  3. Check Gmail for unread from known entities (if asked)
  4. Present prioritized: urgent items > deadlined tasks > pending tasks > email threads
  5. Offer to store new items as tasks (ask first, don't auto-store)

- When fetching email (user asks "check email", "what's new?"):
  1. Search Gmail for recent/unread messages
  2. Cross-reference senders with known entities in memory
  3. Present summary: new threads, action items found
  4. Ask which items to store before logging anything
  5. For approved items: `helix_log_interaction` (source: "email")
  6. For action items: offer to create as tasks linked to relevant project

- When logging interactions with action items:
  Always offer: "Want me to create these as tasks so they show up in your explorer?"

## When NOT to store:
- Casual conversation, greetings
- Questions answered immediately with no lasting value
- Technical debugging steps (unless a decision or pattern emerges)
- NEVER store without asking — present findings, let user choose what sticks
"""
