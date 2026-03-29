"""System tools — compaction and force refresh."""


async def helix_compact(ctx: dict, aggressive: bool = False) -> dict:
    """Run memory maintenance.

    Archives old interactions, decays importance, flags stale entities,
    re-syncs names, regenerates world state, creates backup.

    Args:
        ctx: Context dict with initialized instances
        aggressive: If True, prunes >90 day archived items from Qdrant

    Returns: compaction report dict
    """
    compactor = ctx["compactor"]
    report = await compactor.compact(aggressive=aggressive)
    return report


async def helix_force_refresh(ctx: dict) -> str:
    """Force world state regeneration regardless of staleness.

    Returns: regenerated world state document (markdown)
    """
    ws = await ctx["world_state_gen"].generate(force=True)
    return ws.document
