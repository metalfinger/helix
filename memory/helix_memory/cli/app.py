"""
helix_memory.cli.app

Typer app for the `hmem` CLI. Commands: status, search, entity (subgroup),
timeline, remember, log, compact, refresh, instructions, backup, restore,
health, version.
"""

from __future__ import annotations

import asyncio
import json
import sys
from typing import Optional

import typer
from rich.console import Console
from rich.panel import Panel
from rich.table import Table
from rich.text import Text

# ---------------------------------------------------------------------------
# App setup
# ---------------------------------------------------------------------------

app = typer.Typer(
    help="helix-memory — Personal AI Memory CLI",
    no_args_is_help=True,
)
entity_app = typer.Typer(help="Entity management commands")
app.add_typer(entity_app, name="entity")

console = Console()
err_console = Console(stderr=True)

# Module-level output flags (set by callback)
_state: dict = {
    "json_output": False,
    "verbose": False,
    "quiet": False,
}


# ---------------------------------------------------------------------------
# Global callback (flags)
# ---------------------------------------------------------------------------

@app.callback()
def main(
    json_output: bool = typer.Option(False, "--json", help="Output as JSON"),
    verbose: bool = typer.Option(False, "--verbose", "-v", help="Verbose output"),
    quiet: bool = typer.Option(False, "--quiet", "-q", help="Minimal output"),
) -> None:
    _state["json_output"] = json_output
    _state["verbose"] = verbose
    _state["quiet"] = quiet


# ---------------------------------------------------------------------------
# Async bridge
# ---------------------------------------------------------------------------

def _run(coro):
    """Run an async tool function from a sync Typer command."""
    return asyncio.run(coro)


# ---------------------------------------------------------------------------
# Context initializer
# ---------------------------------------------------------------------------

def _get_ctx() -> dict:
    """Initialize and return the ctx dict with all core modules."""
    from helix_memory.config import settings
    from helix_memory.core.backup import BackupManager
    from helix_memory.core.compactor import MemoryCompactor
    from helix_memory.core.resolver import EntityResolver
    from helix_memory.core.search import MemorySearch
    from helix_memory.core.store import HelixStore
    from helix_memory.core.world_state_gen import WorldStateGenerator

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


# ---------------------------------------------------------------------------
# Output helpers
# ---------------------------------------------------------------------------

def _print_json(data) -> None:
    print(json.dumps(data, default=str, indent=2))


def _out(msg: str) -> None:
    """Print unless --quiet suppresses general output."""
    if not _state["quiet"]:
        console.print(msg)


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------

@app.command()
def status() -> None:
    """Show current world state (active projects, deadlines, blockers)."""
    from helix_memory.tools import helix_get_world_state

    ctx = _get_ctx()
    try:
        document = _run(helix_get_world_state(ctx))
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json({"document": document})
    else:
        console.print(Panel(document, title="World State", border_style="cyan"))


@app.command()
def search(
    query: str = typer.Argument(..., help="Search query"),
    scope: Optional[str] = typer.Option(None, "--scope", help="Scope filter (work|personal|global)"),
    type: Optional[str] = typer.Option(None, "--type", help="Entity type filter (project|person|client|tool|concept)"),
    limit: int = typer.Option(5, "--limit", help="Max results"),
) -> None:
    """Semantic search across all memory."""
    from helix_memory.tools import helix_search_memory

    ctx = _get_ctx()
    types = [type] if type else None
    try:
        results = _run(helix_search_memory(ctx, query, scope=scope, types=types, limit=limit))
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json(results)
        return

    if not results:
        _out("[yellow]No results found.[/yellow]")
        return

    table = Table(title=f"Search: {query!r}", show_lines=True)
    table.add_column("Type", style="dim", width=12)
    table.add_column("Name / Summary", style="bold")
    table.add_column("Score", width=7)
    table.add_column("Details")

    for r in results:
        if r["type"] == "entity":
            details = f"[dim]{r.get('entity_type', '')}[/dim]  {r.get('status', '')}  {r.get('context', '')}"
            table.add_row("entity", r["name"], str(r["score"]), details)
        else:
            details = f"[dim]source:[/dim] {r.get('source', '')}  {r.get('timestamp', '')}"
            table.add_row("interaction", r["summary"][:80], str(r["score"]), details)

    console.print(table)


@app.command()
def timeline(
    entity_name: str = typer.Argument(..., help="Entity name"),
    days: int = typer.Option(14, "--days", help="How many days back to look"),
) -> None:
    """Show chronological interaction history for an entity."""
    from helix_memory.tools import helix_get_timeline

    ctx = _get_ctx()
    try:
        entries = _run(helix_get_timeline(ctx, entity_name, days=days))
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json(entries)
        return

    if not entries:
        _out(f"[yellow]No timeline entries found for {entity_name!r}.[/yellow]")
        return

    console.print(Panel(f"Timeline: [bold]{entity_name}[/bold] — last {days} days", border_style="blue"))
    for entry in entries:
        if "note" in entry:
            console.print(f"[yellow]Note:[/yellow] {entry['note']}")
            continue
        if "error" in entry:
            console.print(f"[red]Error:[/red] {entry['error']}")
            continue
        ts = entry.get("timestamp", "")
        src = entry.get("source", "")
        importance = entry.get("importance", "")
        summary = entry.get("summary", "")
        console.print(f"  [dim]{ts}[/dim]  [cyan]{src}[/cyan]  [dim]importance={importance}[/dim]")
        console.print(f"    {summary}")
        if "action_items" in entry:
            for ai in entry["action_items"]:
                status_marker = "[green]✓[/green]" if ai.get("status") == "done" else "[yellow]○[/yellow]"
                console.print(f"    {status_marker} {ai['description']}")
        console.print()


@app.command()
def remember(
    text: str = typer.Argument(..., help="Content to remember"),
    type: str = typer.Option("note", "--type", help="Type: decision|note|preference|action_item|context|status_update"),
    related_to: Optional[str] = typer.Option(None, "--related-to", help="Comma-separated entity names to link"),
    importance: Optional[float] = typer.Option(None, "--importance", help="Importance 0.0-1.0"),
) -> None:
    """Store something important in memory."""
    from helix_memory.tools import helix_remember

    ctx = _get_ctx()
    related_list = [name.strip() for name in related_to.split(",")] if related_to else None

    try:
        result = _run(helix_remember(ctx, text, type=type, related_to=related_list, importance=importance))
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json(result)
        return

    console.print(f"[green]Stored[/green] interaction [dim]{result['interaction_id']}[/dim]")
    if result.get("entities_linked"):
        console.print(f"  Linked: {', '.join(result['entities_linked'])}")
    if result.get("warning"):
        console.print(f"  [yellow]Warning:[/yellow] {result['warning']}")


@app.command(name="log")
def log_interaction(
    summary: str = typer.Argument(..., help="What happened"),
    source: str = typer.Option("manual", "--source", help="Source: meeting|call|in_person|manual"),
    entities: Optional[str] = typer.Option(None, "--entities", help="Comma-separated entity names"),
    importance: float = typer.Option(0.6, "--importance", help="Importance 0.0-1.0"),
) -> None:
    """Log an interaction (meeting, call, event)."""
    from helix_memory.tools import helix_log_interaction

    ctx = _get_ctx()
    entity_list = [e.strip() for e in entities.split(",")] if entities else None

    try:
        result = _run(helix_log_interaction(ctx, summary, source=source, entities=entity_list, importance=importance))
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json(result)
        return

    console.print(f"[green]Logged[/green] interaction [dim]{result['interaction_id']}[/dim]")
    if result.get("entities_linked"):
        console.print(f"  Linked: {', '.join(result['entities_linked'])}")
    if result.get("action_items_created"):
        console.print(f"  Action items: {len(result['action_items_created'])}")


@app.command()
def compact(
    aggressive: bool = typer.Option(False, "--aggressive", help="Prune archived items older than 90 days"),
) -> None:
    """Run memory maintenance (archive, decay, compact)."""
    from helix_memory.tools import helix_compact

    ctx = _get_ctx()
    if not _state["quiet"]:
        console.print("[cyan]Running compaction...[/cyan]")

    try:
        report = _run(helix_compact(ctx, aggressive=aggressive))
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json(report)
        return

    console.print("[green]Compaction complete.[/green]")
    for key, value in report.items():
        console.print(f"  {key}: {value}")


@app.command()
def refresh() -> None:
    """Force world state regeneration regardless of staleness."""
    from helix_memory.tools import helix_force_refresh

    ctx = _get_ctx()
    if not _state["quiet"]:
        console.print("[cyan]Regenerating world state...[/cyan]")

    try:
        document = _run(helix_force_refresh(ctx))
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json({"document": document})
    else:
        console.print(Panel(document, title="World State (refreshed)", border_style="green"))


@app.command()
def instructions() -> None:
    """Show instructions for LLM integration."""
    from helix_memory.tools import helix_get_instructions

    ctx = _get_ctx()
    try:
        text = _run(helix_get_instructions(ctx))
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json({"instructions": text})
    else:
        console.print(Panel(text, title="Helix Memory — Integration Instructions", border_style="magenta"))


@app.command()
def backup() -> None:
    """Create a backup of all memory data."""
    ctx = _get_ctx()
    backup_manager = ctx["backup_manager"]

    if not _state["quiet"]:
        console.print("[cyan]Creating backup...[/cyan]")

    try:
        backup_path = _run(backup_manager.export_backup())
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json({"backup_path": str(backup_path)})
    else:
        console.print(f"[green]Backup saved:[/green] {backup_path}")


@app.command()
def restore(
    backup_file: str = typer.Argument(..., help="Path to backup file"),
) -> None:
    """Restore memory from a backup file."""
    ctx = _get_ctx()
    backup_manager = ctx["backup_manager"]

    if not _state["quiet"]:
        console.print(f"[cyan]Restoring from {backup_file!r}...[/cyan]")

    try:
        result = _run(backup_manager.restore_from_backup(backup_file))
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json(result)
        return

    console.print("[green]Restore complete.[/green]")
    for key, value in result.items():
        console.print(f"  {key}: {value}")


@app.command()
def health() -> None:
    """Check Qdrant connection and collection health."""
    from helix_memory.config import settings
    from helix_memory.core.store import HelixStore

    store = HelixStore(
        qdrant_url=settings.qdrant.url,
        collection_prefix=settings.qdrant.collection_prefix,
        api_key=settings.qdrant.api_key,
    )

    async def _check() -> dict:
        try:
            entity_count = await store.count_entities()
            interaction_count = await store.count_interactions()
            await store.close()
            return {
                "status": "ok",
                "qdrant_url": settings.qdrant.url,
                "collection_prefix": settings.qdrant.collection_prefix,
                "entity_count": entity_count,
                "interaction_count": interaction_count,
            }
        except Exception as exc:
            return {
                "status": "error",
                "qdrant_url": settings.qdrant.url,
                "error": str(exc),
            }

    result = _run(_check())

    if _state["json_output"]:
        _print_json(result)
        return

    status_color = "green" if result["status"] == "ok" else "red"
    console.print(f"Status: [{status_color}]{result['status']}[/{status_color}]")
    console.print(f"  Qdrant URL:    {result['qdrant_url']}")
    if result["status"] == "ok":
        console.print(f"  Entities:      {result['entity_count']}")
        console.print(f"  Interactions:  {result['interaction_count']}")
    else:
        console.print(f"  [red]Error:[/red] {result.get('error', 'unknown')}")

    if result["status"] != "ok":
        raise typer.Exit(1)


@app.command()
def version() -> None:
    """Show helix-memory version."""
    from helix_memory import __version__

    if _state["json_output"]:
        _print_json({"version": __version__})
    else:
        print(f"helix-memory {__version__}")


# ---------------------------------------------------------------------------
# Entity sub-commands
# ---------------------------------------------------------------------------

@entity_app.command(name="get")
def entity_get(
    name: str = typer.Argument(..., help="Entity name (fuzzy matched)"),
) -> None:
    """Get full details about an entity."""
    from helix_memory.tools import helix_get_entity

    ctx = _get_ctx()
    try:
        result = _run(helix_get_entity(ctx, name))
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json(result)
        return

    if "error" in result:
        console.print(f"[red]Error:[/red] {result['error']}")
        raise typer.Exit(1)

    if result.get("disambiguation"):
        console.print(f"[yellow]Ambiguous name {name!r} — matches:[/yellow]")
        for m in result["matches"]:
            console.print(f"  [bold]{m['name']}[/bold]  ({m['type']}, {m['scope']}, {m['status']})")
            console.print(f"    {m['context']}")
        return

    # Full entity display
    console.print(Panel(
        f"[bold]{result['name']}[/bold]  [dim]{result['type']} / {result['scope']}[/dim]",
        border_style="blue",
    ))
    console.print(f"  Status:   {result['status']}    Priority: {result['priority']}")
    console.print(f"  Created:  {result['created_at']}    Updated: {result['updated_at']}")
    if result.get("tags"):
        console.print(f"  Tags:     {', '.join(result['tags'])}")
    if result.get("aliases"):
        console.print(f"  Aliases:  {', '.join(result['aliases'])}")
    console.print()
    console.print(result.get("context", ""))

    if result.get("relations"):
        console.print("\n[bold]Relations[/bold]")
        for rel in result["relations"]:
            console.print(f"  {rel['type']} → {rel['target_name']}  [dim]{rel.get('detail', '')}[/dim]")

    if result.get("recent_interactions"):
        console.print("\n[bold]Recent Interactions[/bold]")
        for i in result["recent_interactions"]:
            console.print(f"  [dim]{i['timestamp']}[/dim]  {i['summary']}")


@entity_app.command(name="list")
def entity_list(
    type: Optional[str] = typer.Option(None, "--type", help="Filter by entity type"),
    scope: Optional[str] = typer.Option(None, "--scope", help="Filter by scope"),
    status: str = typer.Option("active", "--status", help="Status filter (active|archived|all)"),
) -> None:
    """List entities, optionally filtered."""
    from helix_memory.tools import helix_list_entities

    ctx = _get_ctx()
    try:
        entities = _run(helix_list_entities(ctx, type=type, scope=scope, status=status))
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json(entities)
        return

    if not entities:
        _out("[yellow]No entities found.[/yellow]")
        return

    table = Table(title=f"Entities ({len(entities)})", show_lines=False)
    table.add_column("Name", style="bold")
    table.add_column("Type", style="dim")
    table.add_column("Scope", style="dim")
    table.add_column("Status")
    table.add_column("Priority")
    table.add_column("Last Interaction", style="dim")

    for e in entities:
        table.add_row(
            e["name"],
            e["type"],
            e["scope"],
            e["status"],
            e.get("priority", ""),
            e.get("last_interaction", ""),
        )

    console.print(table)


@entity_app.command(name="create")
def entity_create(
    type: str = typer.Option(..., "--type", help="Entity type: project|person|client|tool|decision|concept"),
    name: str = typer.Option(..., "--name", help="Entity name"),
    scope: str = typer.Option("global", "--scope", help="Scope: work|personal|global"),
    context: str = typer.Option("", "--context", help="Descriptive context"),
    priority: str = typer.Option("medium", "--priority", help="Priority: critical|high|medium|low"),
    tags: Optional[str] = typer.Option(None, "--tags", help="Comma-separated tags"),
    aliases: Optional[str] = typer.Option(None, "--aliases", help="Comma-separated aliases"),
) -> None:
    """Create a new entity."""
    from helix_memory.tools import helix_create_entity

    ctx = _get_ctx()
    tag_list = [t.strip() for t in tags.split(",")] if tags else None
    alias_list = [a.strip() for a in aliases.split(",")] if aliases else None

    try:
        result = _run(helix_create_entity(
            ctx,
            type=type,
            name=name,
            scope=scope,
            context=context,
            priority=priority,
            tags=tag_list,
            aliases=alias_list,
        ))
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json(result)
        return

    if "error" in result:
        console.print(f"[red]Error:[/red] {result['error']}")
        raise typer.Exit(1)

    entity = result["entity"]
    console.print(f"[green]Created[/green] entity [bold]{entity['name']}[/bold] [dim]({entity['id']})[/dim]")
    if result.get("warning"):
        console.print(f"  [yellow]Warning:[/yellow] {result['warning']}")


@entity_app.command(name="update")
def entity_update(
    name: str = typer.Argument(..., help="Entity name (fuzzy matched)"),
    status: Optional[str] = typer.Option(None, "--status", help="New status"),
    priority: Optional[str] = typer.Option(None, "--priority", help="New priority"),
    context: Optional[str] = typer.Option(None, "--context", help="New context (replaces existing)"),
    tags: Optional[str] = typer.Option(None, "--tags", help="New tags (comma-separated, replaces existing)"),
) -> None:
    """Update an entity's fields."""
    from helix_memory.tools import helix_update_entity

    ctx = _get_ctx()
    updates: dict = {}
    if status is not None:
        updates["status"] = status
    if priority is not None:
        updates["priority"] = priority
    if context is not None:
        updates["context"] = context
    if tags is not None:
        updates["tags"] = [t.strip() for t in tags.split(",")]

    if not updates:
        err_console.print("[yellow]No update fields provided. Use --status, --priority, --context, or --tags.[/yellow]")
        raise typer.Exit(1)

    try:
        result = _run(helix_update_entity(ctx, name, updates))
    except Exception as exc:
        err_console.print(f"[red]Error:[/red] {exc}")
        raise typer.Exit(1)

    if _state["json_output"]:
        _print_json(result)
        return

    if "error" in result:
        console.print(f"[red]Error:[/red] {result['error']}")
        if "candidates" in result:
            for c in result["candidates"]:
                console.print(f"  [dim]{c['name']}[/dim] ({c['type']})")
        raise typer.Exit(1)

    console.print(f"[green]Updated[/green] entity [bold]{result.get('name', name)}[/bold]")
    for field in updates:
        console.print(f"  {field}: {result.get(field, updates[field])}")
