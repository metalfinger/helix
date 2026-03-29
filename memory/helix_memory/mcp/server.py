"""
helix_memory.mcp.server

MCP server entrypoint. Registers all tool handlers from helix_memory.tools,
initialises the Qdrant store and embeddings, and starts the stdio transport.
Populated in Epic 5b (E5b).
"""

from __future__ import annotations

import asyncio
import json
import logging
from contextlib import asynccontextmanager
from typing import Any

from mcp import types
from mcp.server import Server
from mcp.server.stdio import stdio_server

from helix_memory.config import settings
from helix_memory.anatomy import get_anatomy
from helix_memory.core.embeddings import embed
from helix_memory.tools import (
    helix_get_world_state,
    helix_search_memory,
    helix_get_entity,
    helix_get_timeline,
    helix_list_entities,
    helix_list_tasks,
    helix_get_instructions,
    helix_remember,
    helix_update_entity,
    helix_create_entity,
    helix_log_interaction,
    helix_create_task,
    helix_complete_task,
    helix_update_task,
    helix_delete_entity,
    helix_compact,
    helix_force_refresh,
    helix_add_payment,
    helix_set_salary,
    helix_mark_paid,
    helix_finance_summary,
)

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Tool definitions — name, description, JSON Schema
# ---------------------------------------------------------------------------

_TOOL_DEFS: list[types.Tool] = [
    types.Tool(
        name="helix_get_world_state",
        description=(
            "Get the current situation — active projects, urgent items, deadlines, blockers. "
            "CALL THIS FIRST in every new session."
        ),
        inputSchema={
            "type": "object",
            "properties": {},
            "required": [],
        },
    ),
    types.Tool(
        name="helix_search_memory",
        description="Semantic search across all knowledge — projects, people, decisions, past interactions.",
        inputSchema={
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Free-text search query.",
                },
                "scope": {
                    "type": "string",
                    "enum": ["work", "personal", "global"],
                    "description": "Optional scope filter.",
                },
                "types": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional entity type filter — e.g. [\"project\", \"person\"].",
                },
                "time_range": {
                    "type": "string",
                    "enum": ["today", "this_week", "this_month"],
                    "description": "Optional interaction time filter.",
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default 5).",
                    "default": 5,
                },
            },
            "required": ["query"],
        },
    ),
    types.Tool(
        name="helix_get_entity",
        description=(
            "Full details about a project, person, client, or concept. "
            "Includes context, relations, and last 5 interactions."
        ),
        inputSchema={
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Entity name (fuzzy matched via resolver).",
                },
            },
            "required": ["name"],
        },
    ),
    types.Tool(
        name="helix_get_timeline",
        description="Chronological interaction history for an entity.",
        inputSchema={
            "type": "object",
            "properties": {
                "entity_name": {
                    "type": "string",
                    "description": "Entity name (fuzzy matched via resolver).",
                },
                "days": {
                    "type": "integer",
                    "description": "How many days back to look (default 14).",
                    "default": 14,
                },
            },
            "required": ["entity_name"],
        },
    ),
    types.Tool(
        name="helix_list_entities",
        description="List all entities, optionally filtered.",
        inputSchema={
            "type": "object",
            "properties": {
                "type": {
                    "type": "string",
                    "description": "Optional entity type filter — e.g. \"project\", \"person\", \"client\".",
                },
                "scope": {
                    "type": "string",
                    "enum": ["work", "personal", "global"],
                    "description": "Optional scope filter.",
                },
                "status": {
                    "type": "string",
                    "description": "Status filter (default \"active\"). Use \"all\" to skip.",
                    "default": "active",
                },
            },
            "required": [],
        },
    ),
    types.Tool(
        name="helix_get_instructions",
        description="Returns instructions for how to use helix-memory tools effectively.",
        inputSchema={
            "type": "object",
            "properties": {},
            "required": [],
        },
    ),
    types.Tool(
        name="helix_remember",
        description="Store something important. helix-memory resolves entity names, links relations, and stores as interaction.",
        inputSchema={
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The content to remember.",
                },
                "type": {
                    "type": "string",
                    "enum": ["decision", "note", "preference", "action_item", "context", "status_update"],
                    "description": "Interaction type (default \"note\").",
                    "default": "note",
                },
                "related_to": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional list of entity names to link.",
                },
                "importance": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "description": "Override importance 0.0-1.0; defaults by type if omitted.",
                },
            },
            "required": ["content"],
        },
    ),
    types.Tool(
        name="helix_update_entity",
        description="Update a project, person, or concept. Context is REWRITTEN (not appended).",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Entity name (fuzzy matched).",
                },
                "updates": {
                    "type": "object",
                    "description": (
                        "Fields to update. Supported: status, priority, context, tags, "
                        "aliases, scope, name, type."
                    ),
                },
            },
            "required": ["name", "updates"],
        },
    ),
    types.Tool(
        name="helix_create_entity",
        description="Create a new entity with full control over fields. Runs dedupe check.",
        inputSchema={
            "type": "object",
            "properties": {
                "type": {
                    "type": "string",
                    "enum": ["project", "person", "client", "tool", "decision", "concept", "task"],
                    "description": "Entity type.",
                },
                "name": {
                    "type": "string",
                    "description": "Entity name.",
                },
                "aliases": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional alternate names.",
                },
                "scope": {
                    "type": "string",
                    "enum": ["work", "personal", "global"],
                    "description": "Scope (default \"global\").",
                    "default": "global",
                },
                "context": {
                    "type": "string",
                    "description": "Descriptive context string (truncated to 500 words).",
                    "default": "",
                },
                "priority": {
                    "type": "string",
                    "enum": ["critical", "high", "medium", "low"],
                    "description": "Priority (default \"medium\").",
                    "default": "medium",
                },
                "relations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "target_name": {"type": "string"},
                            "type": {"type": "string"},
                            "detail": {"type": "string"},
                        },
                        "required": ["target_name"],
                    },
                    "description": "Optional relations to other entities.",
                },
                "tags": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional tags.",
                },
            },
            "required": ["type", "name"],
        },
    ),
    types.Tool(
        name="helix_log_interaction",
        description="Log an event from outside the current session — meeting, call, conversation.",
        inputSchema={
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "What happened.",
                },
                "source": {
                    "type": "string",
                    "enum": ["email", "mattermost", "telegram", "claude_code", "plane", "outline", "manual", "meeting", "call", "in_person", "system"],
                    "description": "Interaction source (default \"manual\").",
                    "default": "manual",
                },
                "entities": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional entity names to link.",
                },
                "action_items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "description": {"type": "string"},
                            "assignee": {"type": "string"},
                            "assignee_name": {"type": "string"},
                            "deadline": {"type": "string", "description": "ISO date string (YYYY-MM-DD)."},
                        },
                        "required": ["description"],
                    },
                    "description": "Optional action items.",
                },
                "importance": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "description": "Importance 0.0-1.0 (default 0.6).",
                    "default": 0.6,
                },
            },
            "required": ["summary"],
        },
    ),
    types.Tool(
        name="helix_delete_entity",
        description=(
            "Permanently delete an entity from memory. DESTRUCTIVE. "
            "First call without confirm to see what will be deleted, "
            "then call with confirm=True after user approves."
        ),
        inputSchema={
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Entity name (fuzzy matched).",
                },
                "confirm": {
                    "type": "boolean",
                    "description": "Must be True to actually delete. False returns info for confirmation.",
                    "default": False,
                },
            },
            "required": ["name"],
        },
    ),
    types.Tool(
        name="helix_compact",
        description=(
            "Run memory maintenance. Archives old interactions, decays importance, "
            "flags stale entities."
        ),
        inputSchema={
            "type": "object",
            "properties": {
                "aggressive": {
                    "type": "boolean",
                    "description": "If True, prunes >90 day archived items from Qdrant.",
                    "default": False,
                },
            },
            "required": [],
        },
    ),
    types.Tool(
        name="helix_force_refresh",
        description="Force world state regeneration regardless of staleness.",
        inputSchema={
            "type": "object",
            "properties": {},
            "required": [],
        },
    ),
    types.Tool(
        name="helix_create_task",
        description=(
            "Create a task linked to a project. IMPORTANT: If the project doesn't exist, "
            "the tool will return a WARNING and the task will NOT be linked. "
            "If the user mentions a project you haven't seen before, ASK them if you should "
            "create it first (using helix_create_entity) before creating the task. "
            "Auto-links to project via belongs_to relation."
        ),
        inputSchema={
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Task title (becomes entity name).",
                },
                "project": {
                    "type": "string",
                    "description": "Optional project name to link via belongs_to relation.",
                },
                "description": {
                    "type": "string",
                    "description": "Task description (becomes entity context).",
                    "default": "",
                },
                "priority": {
                    "type": "string",
                    "enum": ["critical", "high", "medium", "low"],
                    "description": "Priority (default \"medium\").",
                    "default": "medium",
                },
                "deadline": {
                    "type": "string",
                    "description": "Optional ISO date string (YYYY-MM-DD).",
                },
                "scope": {
                    "type": "string",
                    "enum": ["work", "personal", "global"],
                    "description": "Scope (default \"global\").",
                    "default": "global",
                },
                "linked_entities": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional list of other entity names to link.",
                },
            },
            "required": ["title"],
        },
    ),
    types.Tool(
        name="helix_complete_task",
        description="Mark a task as completed. Logs a task_update interaction and regenerates world state.",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Task name (fuzzy matched).",
                },
                "note": {
                    "type": "string",
                    "description": "Optional completion note appended to context.",
                    "default": "",
                },
            },
            "required": ["name"],
        },
    ),
    types.Tool(
        name="helix_update_task",
        description=(
            "Update a task — change priority, deadline, description, status, or project link. "
            "Use when user says 'change task priority', 'move deadline', 'reassign task to project X', "
            "'pause task', etc."
        ),
        inputSchema={
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Task name (fuzzy matched).",
                },
                "priority": {
                    "type": "string",
                    "enum": ["critical", "high", "medium", "low"],
                    "description": "New priority.",
                },
                "deadline": {
                    "type": "string",
                    "description": "New deadline (YYYY-MM-DD), or 'none' to remove.",
                },
                "description": {
                    "type": "string",
                    "description": "New description.",
                },
                "status": {
                    "type": "string",
                    "enum": ["active", "paused", "completed", "archived"],
                    "description": "New status.",
                },
                "project": {
                    "type": "string",
                    "description": "New project to link to (replaces current).",
                },
                "add_linked": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Additional entities to link.",
                },
            },
            "required": ["name"],
        },
    ),
    types.Tool(
        name="helix_list_tasks",
        description="List tasks, optionally filtered by project, status, or scope.",
        inputSchema={
            "type": "object",
            "properties": {
                "project": {
                    "type": "string",
                    "description": "Optional project name — filters to tasks belonging to this project.",
                },
                "status": {
                    "type": "string",
                    "enum": ["active", "completed", "all"],
                    "description": "Status filter (default \"active\").",
                    "default": "active",
                },
                "scope": {
                    "type": "string",
                    "enum": ["work", "personal", "global"],
                    "description": "Optional scope filter.",
                },
            },
            "required": [],
        },
    ),
    types.Tool(
        name="helix_get_anatomy",
        description=(
            "Get the anatomy map of a project — file descriptions, key symbols, "
            "token estimates, languages. Call BEFORE reading files to understand "
            "the codebase structure and pick the right files to read. Saves tokens "
            "by avoiding unnecessary reads."
        ),
        inputSchema={
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Project directory path. Defaults to current working directory.",
                },
                "query": {
                    "type": "string",
                    "description": "Filter — match filenames, descriptions, symbols, or language.",
                },
                "path_filter": {
                    "type": "string",
                    "description": "Path prefix filter (e.g. 'src/auth' to only show that directory).",
                },
            },
            "required": [],
        },
    ),
    types.Tool(
        name="helix_add_payment",
        description=(
            "Add a payment/invoice for a project. Track freelance income with amounts, "
            "due dates, and installment info."
        ),
        inputSchema={
            "type": "object",
            "properties": {
                "project": {"type": "string", "description": "Project name to link to."},
                "amount": {"type": "integer", "description": "Payment amount (integer, e.g. 50000)."},
                "due_date": {"type": "string", "description": "Due date (YYYY-MM-DD)."},
                "label": {"type": "string", "description": "Label (e.g. 'Advance', 'Final', 'Milestone 1')."},
                "currency": {"type": "string", "default": "INR", "description": "Currency code (default INR)."},
                "scope": {"type": "string", "enum": ["work", "personal", "global"], "default": "personal"},
                "installment": {"type": "string", "description": "Installment info (e.g. '1 of 3')."},
            },
            "required": ["project", "amount"],
        },
    ),
    types.Tool(
        name="helix_set_salary",
        description="Set or update monthly salary. Creates/updates the salary payment entity.",
        inputSchema={
            "type": "object",
            "properties": {
                "amount": {"type": "integer", "description": "Monthly salary amount."},
                "source": {"type": "string", "default": "Employer", "description": "Employer name."},
                "pay_day": {"type": "integer", "default": 1, "description": "Day of month salary is received."},
                "currency": {"type": "string", "default": "INR"},
                "scope": {"type": "string", "enum": ["work", "personal", "global"], "default": "work"},
            },
            "required": ["amount"],
        },
    ),
    types.Tool(
        name="helix_mark_paid",
        description="Mark a payment as received. Updates status to completed and records the paid date.",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Payment name (fuzzy matched)."},
                "paid_date": {"type": "string", "description": "Date received (YYYY-MM-DD). Defaults to today."},
                "note": {"type": "string", "description": "Optional note."},
            },
            "required": ["name"],
        },
    ),
    types.Tool(
        name="helix_finance_summary",
        description="Get financial summary — this month's income, upcoming payments, overdue, and year-to-date totals.",
        inputSchema={
            "type": "object",
            "properties": {},
            "required": [],
        },
    ),
]

# ---------------------------------------------------------------------------
# Dispatch table — maps tool name → (tool_fn, required_args_from_schema)
# ---------------------------------------------------------------------------

def _result_to_text(result: Any) -> str:
    """Serialise a tool return value to a JSON string for TextContent."""
    if isinstance(result, str):
        return result
    return json.dumps(result, ensure_ascii=False, default=str)


async def _dispatch(ctx: dict, name: str, arguments: dict) -> str:
    """Call the appropriate tool function and return serialised output."""
    if name == "helix_get_world_state":
        return _result_to_text(await helix_get_world_state(ctx))

    elif name == "helix_search_memory":
        return _result_to_text(await helix_search_memory(
            ctx,
            query=arguments["query"],
            scope=arguments.get("scope"),
            types=arguments.get("types"),
            time_range=arguments.get("time_range"),
            limit=arguments.get("limit", 5),
        ))

    elif name == "helix_get_entity":
        return _result_to_text(await helix_get_entity(ctx, name=arguments["name"]))

    elif name == "helix_get_timeline":
        return _result_to_text(await helix_get_timeline(
            ctx,
            entity_name=arguments["entity_name"],
            days=arguments.get("days", 14),
        ))

    elif name == "helix_list_entities":
        return _result_to_text(await helix_list_entities(
            ctx,
            type=arguments.get("type"),
            scope=arguments.get("scope"),
            status=arguments.get("status", "active"),
        ))

    elif name == "helix_get_instructions":
        return _result_to_text(await helix_get_instructions(ctx))

    elif name == "helix_remember":
        return _result_to_text(await helix_remember(
            ctx,
            content=arguments["content"],
            type=arguments.get("type", "note"),
            related_to=arguments.get("related_to"),
            importance=arguments.get("importance"),
        ))

    elif name == "helix_update_entity":
        return _result_to_text(await helix_update_entity(
            ctx,
            name=arguments["name"],
            updates=arguments["updates"],
        ))

    elif name == "helix_create_entity":
        return _result_to_text(await helix_create_entity(
            ctx,
            type=arguments["type"],
            name=arguments["name"],
            aliases=arguments.get("aliases"),
            scope=arguments.get("scope", "global"),
            context=arguments.get("context", ""),
            priority=arguments.get("priority", "medium"),
            relations=arguments.get("relations"),
            tags=arguments.get("tags"),
        ))

    elif name == "helix_log_interaction":
        return _result_to_text(await helix_log_interaction(
            ctx,
            summary=arguments["summary"],
            source=arguments.get("source", "manual"),
            entities=arguments.get("entities"),
            action_items=arguments.get("action_items"),
            importance=arguments.get("importance", 0.6),
        ))

    elif name == "helix_delete_entity":
        return _result_to_text(await helix_delete_entity(
            ctx,
            name=arguments["name"],
            confirm=arguments.get("confirm", False),
        ))

    elif name == "helix_compact":
        return _result_to_text(await helix_compact(
            ctx,
            aggressive=arguments.get("aggressive", False),
        ))

    elif name == "helix_force_refresh":
        return _result_to_text(await helix_force_refresh(ctx))

    elif name == "helix_create_task":
        return _result_to_text(await helix_create_task(
            ctx,
            title=arguments["title"],
            project=arguments.get("project"),
            description=arguments.get("description", ""),
            priority=arguments.get("priority", "medium"),
            deadline=arguments.get("deadline"),
            scope=arguments.get("scope", "global"),
            linked_entities=arguments.get("linked_entities"),
        ))

    elif name == "helix_complete_task":
        return _result_to_text(await helix_complete_task(
            ctx,
            name=arguments["name"],
            note=arguments.get("note", ""),
        ))

    elif name == "helix_update_task":
        return _result_to_text(await helix_update_task(
            ctx,
            name=arguments["name"],
            priority=arguments.get("priority"),
            deadline=arguments.get("deadline"),
            description=arguments.get("description"),
            status=arguments.get("status"),
            project=arguments.get("project"),
            add_linked=arguments.get("add_linked"),
        ))

    elif name == "helix_list_tasks":
        return _result_to_text(await helix_list_tasks(
            ctx,
            project=arguments.get("project"),
            status=arguments.get("status", "active"),
            scope=arguments.get("scope"),
        ))

    elif name == "helix_add_payment":
        return _result_to_text(await helix_add_payment(
            ctx,
            project=arguments["project"],
            amount=arguments["amount"],
            due_date=arguments.get("due_date", ""),
            label=arguments.get("label", ""),
            currency=arguments.get("currency", "INR"),
            scope=arguments.get("scope", "personal"),
            installment=arguments.get("installment", ""),
        ))

    elif name == "helix_set_salary":
        return _result_to_text(await helix_set_salary(
            ctx,
            amount=arguments["amount"],
            source=arguments.get("source", "Employer"),
            pay_day=arguments.get("pay_day", 1),
            currency=arguments.get("currency", "INR"),
            scope=arguments.get("scope", "work"),
        ))

    elif name == "helix_mark_paid":
        return _result_to_text(await helix_mark_paid(
            ctx,
            name=arguments["name"],
            paid_date=arguments.get("paid_date", ""),
            note=arguments.get("note", ""),
        ))

    elif name == "helix_finance_summary":
        return _result_to_text(await helix_finance_summary(ctx))

    elif name == "helix_get_anatomy":
        return _result_to_text(await get_anatomy(
            project_path=arguments.get("path"),
            query=arguments.get("query"),
            path_filter=arguments.get("path_filter"),
        ))

    else:
        raise ValueError(f"Unknown tool: {name}")


# ---------------------------------------------------------------------------
# Context initialisation
# ---------------------------------------------------------------------------

async def _build_ctx() -> dict:
    """Initialise all core modules and return the ctx dict."""
    from helix_memory.core.store import HelixStore
    from helix_memory.core.search import MemorySearch
    from helix_memory.core.resolver import EntityResolver
    from helix_memory.core.world_state_gen import WorldStateGenerator
    from helix_memory.core.compactor import MemoryCompactor
    from helix_memory.core.backup import BackupManager

    store = HelixStore(
        qdrant_url=settings.qdrant.url,
        collection_prefix=settings.qdrant.collection_prefix,
        api_key=settings.qdrant.api_key,
    )
    search = MemorySearch(store)
    resolver = EntityResolver(store)
    world_state_gen = WorldStateGenerator(
        store,
        search,
        stale_hours=settings.memory.world_state_stale_hours,
    )
    backup_manager = BackupManager(store)
    compactor = MemoryCompactor(store, world_state_gen, backup_manager)

    return {
        "store": store,
        "search": search,
        "resolver": resolver,
        "world_state_gen": world_state_gen,
        "compactor": compactor,
        "backup_manager": backup_manager,
    }


async def _run_health_check(ctx: dict) -> None:
    """Attempt a lightweight health check; log warnings but do not block startup."""
    try:
        store = ctx["store"]
        # Verify Qdrant connectivity by listing collections
        await store._client.get_collections()
        logger.info("helix-memory: Qdrant health check passed.")
    except Exception as exc:
        logger.warning(
            "helix-memory: Qdrant health check failed — %s. "
            "Continuing anyway; tool calls may fail until Qdrant is available.",
            exc,
        )


# ---------------------------------------------------------------------------
# Server factory & main
# ---------------------------------------------------------------------------

def build_server(ctx: dict) -> Server:
    """Create and configure the MCP Server with ctx captured in closures."""
    server: Server = Server("helix-memory")

    @server.list_tools()
    async def list_tools() -> list[types.Tool]:
        return _TOOL_DEFS

    @server.call_tool()
    async def call_tool(
        name: str,
        arguments: dict,
    ) -> list[types.TextContent]:
        try:
            text = await _dispatch(ctx, name, arguments)
            return [types.TextContent(type="text", text=text)]
        except Exception as exc:
            logger.exception("Tool %s raised an exception", name)
            error_text = json.dumps({"error": str(exc)})
            return [types.TextContent(type="text", text=error_text)]

    return server


async def _async_main() -> None:
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s — %(message)s",
    )

    logger.info("helix-memory MCP server starting up …")

    ctx = await _build_ctx()
    await _run_health_check(ctx)

    # Pre-warm embedding model so first tool call doesn't block for ~10s
    import concurrent.futures
    loop = asyncio.get_event_loop()
    logger.info("Pre-warming embedding model...")
    await loop.run_in_executor(None, lambda: embed("warmup"))
    logger.info("Embedding model ready.")

    server = build_server(ctx)
    init_options = server.create_initialization_options()

    async with stdio_server() as (read_stream, write_stream):
        await server.run(read_stream, write_stream, init_options)


def main() -> None:
    """Synchronous entry point — called from __main__.py and pyproject scripts."""
    asyncio.run(_async_main())
