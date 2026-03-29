"""World state generation — programmatic, no LLM call.

Fetches entities + interactions from Qdrant, computes structured sections,
renders markdown template, writes atomically to ~/.helix/state/.

DOES NOT use the resolver — reads entities directly via store.scroll().
Key: lazy regeneration. Only regenerates when stale (> stale_hours old).
"""

from __future__ import annotations

import hashlib
import json
import logging
import shutil
from datetime import date, datetime, timedelta
from pathlib import Path
from typing import Optional

from helix_memory.core.file_writer import atomic_write


def _naive_utc(dt: datetime) -> datetime:
    """Strip timezone info from a datetime, making it naive UTC.

    Qdrant returns tz-aware datetimes (parsed from ISO strings with Z suffix).
    Our code uses datetime.utcnow() which returns naive. This helper ensures
    we can always compare them without 'can't compare offset-naive and
    offset-aware datetimes' errors.
    """
    if dt.tzinfo is not None:
        return dt.replace(tzinfo=None)
    return dt
from helix_memory.core.search import MemorySearch
from helix_memory.core.store import HelixStore
from helix_memory.models.entity import WorldEntity
from helix_memory.models.interaction import ActionItem, Interaction
from helix_memory.models.world_state import (
    ActionSummary,
    DeadlineItem,
    DecisionSummary,
    ExplorerActionCard,
    ExplorerData,
    ExplorerDecisionCard,
    ExplorerInteraction,
    ExplorerPaymentCard,
    ExplorerPersonCard,
    ExplorerProjectCard,
    ExplorerTimelineEntry,
    FinanceSummary,
    MonthForecast,
    ProjectSummary,
    StaleItem,
    UrgentItem,
    WaitingItem,
    WorldState,
    WorldStateSections,
)
from helix_memory.utils.platform import get_state_dir
from helix_memory.utils.text_processing import extract_deadlines

logger = logging.getLogger(__name__)

_MONTH_MAP = {
    "january": 1, "february": 2, "march": 3, "april": 4,
    "may": 5, "june": 6, "july": 7, "august": 8,
    "september": 9, "october": 10, "november": 11, "december": 12,
    "jan": 1, "feb": 2, "mar": 3, "apr": 4,
    "jun": 6, "jul": 7, "aug": 8, "sep": 9, "oct": 10, "nov": 11, "dec": 12,
}


def _parse_deadline_tag(tag: str) -> Optional[date]:
    """Parse a deadline-* tag into a date.

    Handles patterns like:
      deadline-april → April of current or next year
      deadline-2025-06-15 → ISO date
      deadline-q2 → last day of Q2 (June 30)
    """
    suffix = tag[len("deadline-"):].lower().strip()
    if not suffix:
        return None

    # Try ISO date first: deadline-2025-06-15
    try:
        return date.fromisoformat(suffix)
    except ValueError:
        pass

    # Try month name: deadline-april
    if suffix in _MONTH_MAP:
        month = _MONTH_MAP[suffix]
        today = date.today()
        year = today.year
        # If that month has already passed this year, use next year
        if month < today.month:
            year += 1
        import calendar
        last_day = calendar.monthrange(year, month)[1]
        return date(year, month, last_day)

    # Try quarter: deadline-q1, deadline-q2, etc.
    if suffix.startswith("q") and len(suffix) == 2 and suffix[1].isdigit():
        q = int(suffix[1])
        if 1 <= q <= 4:
            import calendar
            last_month = q * 3
            today = date.today()
            year = today.year
            if last_month < today.month:
                year += 1
            last_day = calendar.monthrange(year, last_month)[1]
            return date(year, last_month, last_day)

    return None


def _try_parse_date_string(date_str: str) -> Optional[date]:
    """Try to parse a free-form date string into a date. Returns None on failure."""
    import re
    date_str = date_str.strip()

    # Try ISO
    try:
        return date.fromisoformat(date_str)
    except ValueError:
        pass

    # Try "Month Day, Year" or "Month Day Year"
    patterns = [
        r"(\w+)\s+(\d{1,2}),?\s+(\d{4})",  # April 15, 2025
        r"(\d{1,2})\s+(\w+)\s+(\d{4})",     # 15 April 2025
        r"(\d{4})-(\d{2})-(\d{2})",          # 2025-04-15
    ]
    for pattern in patterns:
        m = re.match(pattern, date_str, re.IGNORECASE)
        if m:
            groups = m.groups()
            try:
                if pattern == patterns[0]:
                    month_str, day, year = groups
                    month = _MONTH_MAP.get(month_str.lower())
                    if month:
                        return date(int(year), month, int(day))
                elif pattern == patterns[1]:
                    day, month_str, year = groups
                    month = _MONTH_MAP.get(month_str.lower())
                    if month:
                        return date(int(year), month, int(day))
            except (ValueError, TypeError):
                pass

    return None


class WorldStateGenerator:
    """Programmatic world state generator. No LLM calls."""

    def __init__(
        self,
        store: HelixStore,
        search: MemorySearch,
        stale_hours: int = 6,
    ) -> None:
        self._store = store
        self._search = search
        self._stale_hours = stale_hours

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    async def generate(self, force: bool = False) -> WorldState:
        """Generate world state. Returns cached if fresh, unless force=True.

        Steps:
        1. Check staleness — if current state exists and fresh and not force, return it
        2. Fetch all active entities (grouped by scope + type)
        3. Fetch interactions from last 48 hours
        4. Fetch all pending action items
        5. Compute sections
        6. Render markdown from template
        7. Compute SHA-256 checksum of document
        8. File rotation: current → previous, write new current (atomic)
        9. Prune history/ to last 7
        """
        import time
        t_start = time.monotonic()

        if not force:
            current = self.load_current()
            if current is not None:
                stale_threshold = _naive_utc(current.generated_at) + timedelta(hours=self._stale_hours)
                if datetime.utcnow() < stale_threshold:
                    logger.debug("World state is fresh, returning cached version")
                    return current

        logger.info("Generating world state (force=%s)", force)

        # Fetch data
        all_entities = await self._store.list_all_entities(include_archived=False)
        recent_interactions = await self._store.get_recent_interactions(hours=48)
        all_interactions = await self._store.get_active_interactions()
        pending_pairs = await self._search.get_pending_action_items()
        stale_entities = await self._search.get_stale_entities(days=14)

        # Compute sections
        sections = await self._compute_sections(
            all_entities=all_entities,
            recent_interactions=recent_interactions,
            pending_pairs=pending_pairs,
            stale_entities=stale_entities,
        )

        # Explorer data (richer view for TUI explorer)
        explorer_data = await self._compute_explorer_data(
            all_entities=all_entities,
            all_interactions=all_interactions,
            pending_pairs=pending_pairs,
        )

        entity_count = len(all_entities)
        interaction_count = len(recent_interactions)
        pending_action_count = len(sections.pending_actions)

        # Determine version
        prev = self.load_current()
        version = (prev.version + 1) if prev is not None else 1

        now = datetime.utcnow()
        stale_after = now + timedelta(hours=self._stale_hours)

        # Render markdown
        document = self._render_markdown(sections, now)

        # Compute checksum
        checksum = hashlib.sha256(document.encode("utf-8")).hexdigest()

        t_end = time.monotonic()
        duration_ms = int((t_end - t_start) * 1000)

        world_state = WorldState(
            version=version,
            generated_at=now,
            stale_after=stale_after,
            checksum=checksum,
            entity_count=entity_count,
            interaction_count=interaction_count,
            pending_action_count=pending_action_count,
            document=document,
            sections=sections,
            generation_duration_ms=duration_ms,
            explorer=explorer_data,
        )

        # File rotation and write
        self._rotate_files()
        state_dir = get_state_dir()
        current_path = state_dir / "world_state_current.json"
        atomic_write(current_path, world_state.model_dump(mode="json"))

        logger.info(
            "World state generated: v%d, %d entities, %d interactions, %dms",
            version, entity_count, interaction_count, duration_ms,
        )
        return world_state

    def load_current(self) -> WorldState | None:
        """Load current world state from file, or None if missing/corrupt."""
        state_dir = get_state_dir()
        current_path = state_dir / "world_state_current.json"
        if not current_path.exists():
            return None
        try:
            raw = current_path.read_text(encoding="utf-8")
            data = json.loads(raw)
            return WorldState.model_validate(data)
        except Exception:
            logger.warning("Failed to load current world state — treating as missing")
            return None

    # ------------------------------------------------------------------
    # File rotation
    # ------------------------------------------------------------------

    def _rotate_files(self) -> None:
        """current → previous, save copy to history/"""
        state_dir = get_state_dir()
        current = state_dir / "world_state_current.json"
        previous = state_dir / "world_state_previous.json"
        history_dir = state_dir / "history"
        history_dir.mkdir(parents=True, exist_ok=True)

        if current.exists():
            # Copy current to history with timestamp name
            timestamp = datetime.utcnow().strftime("%Y-%m-%dT%H-%M-%SZ")
            shutil.copy2(current, history_dir / f"{timestamp}.json")

            # Move current to previous
            shutil.copy2(current, previous)

            # Prune history to last 7
            history_files = sorted(history_dir.glob("*.json"), reverse=True)
            for old_file in history_files[7:]:
                old_file.unlink()

    # ------------------------------------------------------------------
    # Section computation
    # ------------------------------------------------------------------

    async def _compute_sections(
        self,
        all_entities: list[WorldEntity],
        recent_interactions: list[Interaction],
        pending_pairs: list[tuple],
        stale_entities: list[WorldEntity],
    ) -> WorldStateSections:
        today = date.today()

        # Set of truly active entity IDs (not completed/archived) — used to filter
        # action items and deadlines from dead/completed projects.
        # all_entities excludes archived, so entity_id NOT in this set means
        # archived or deleted. Completed entities are in the list but excluded.
        active_entity_ids = {e.id for e in all_entities if e.status not in ("completed", "archived")}

        # --- Urgent ---
        urgent: list[UrgentItem] = []
        three_days_from_now = today + timedelta(days=3)

        for entity in all_entities:
            # Skip archived/completed entities
            if entity.id not in active_entity_ids:
                continue
            # Check deadline tags for urgency
            entity_deadline: Optional[date] = None
            for tag in entity.tags:
                if tag.startswith("deadline-"):
                    parsed = _parse_deadline_tag(tag)
                    if parsed and (entity_deadline is None or parsed < entity_deadline):
                        entity_deadline = parsed

            is_deadline_soon = entity_deadline is not None and entity_deadline <= three_days_from_now

            if is_deadline_soon:
                urgent.append(UrgentItem(
                    description=f"{entity.name}: {entity.context}" if entity.context else entity.name,
                    source_entity=entity.id,
                    deadline=entity_deadline,
                    importance=0.9,
                ))

        # Also check pending actions with near deadlines
        for interaction, actions in pending_pairs:
            for action in actions:
                if action.deadline and action.deadline <= three_days_from_now:
                    urgent.append(UrgentItem(
                        description=action.description,
                        source_entity=interaction.id,
                        deadline=action.deadline,
                        importance=0.9,
                    ))

        # --- Projects by scope ---
        def _project_summary(entity: WorldEntity) -> ProjectSummary:
            # Find deadline from tags
            proj_deadline: Optional[date] = None
            for tag in entity.tags:
                if tag.startswith("deadline-"):
                    parsed = _parse_deadline_tag(tag)
                    if parsed and (proj_deadline is None or parsed < proj_deadline):
                        proj_deadline = parsed

            # Also try extract_deadlines on context
            if not proj_deadline and entity.context:
                for ds in extract_deadlines(entity.context):
                    parsed = _try_parse_date_string(ds)
                    if parsed:
                        proj_deadline = parsed
                        break

            # Find blockers from relations
            blockers = [
                r.target_name for r in entity.relations if r.type == "blocked_by"
            ]
            # Find key people
            key_people = [
                r.target_name for r in entity.relations
                if r.type in ("team_member", "works_on", "reports_to")
            ]

            status_line = entity.context[:120] if entity.context else f"Active project — {entity.name}"

            return ProjectSummary(
                entity_id=entity.id,
                name=entity.name,
                status_line=status_line,
                deadline=proj_deadline,
                blockers=blockers,
                key_people=key_people,
            )

        projects_work: list[ProjectSummary] = []
        projects_personal: list[ProjectSummary] = []
        projects_personal: list[ProjectSummary] = []

        for entity in all_entities:
            if entity.type != "project" or entity.status != "active":
                continue
            summary = _project_summary(entity)
            if entity.scope == "work":
                projects_work.append(summary)
            elif entity.scope == "personal":
                projects_personal.append(summary)
            elif entity.scope == "personal":
                projects_personal.append(summary)
            # global scope projects go into personal as fallback
            else:
                projects_personal.append(summary)

        # --- Waiting on ---
        waiting_on: list[WaitingItem] = []
        for entity in all_entities:
            for rel in entity.relations:
                if rel.type == "waiting_on":
                    # Use relation creation date for accurate waiting duration
                    wait_since = rel.created_at.date() if isinstance(rel.created_at, datetime) else today
                    days_waiting = (today - wait_since).days
                    waiting_on.append(WaitingItem(
                        description=rel.detail or f"Waiting on {rel.target_name} re: {entity.name}",
                        from_person=rel.target_name,
                        since=wait_since,
                        days_waiting=days_waiting,
                    ))

        # --- Deadlines ---
        deadlines: list[DeadlineItem] = []

        # From entity tags (skip archived/completed)
        for entity in all_entities:
            if entity.id not in active_entity_ids:
                continue
            for tag in entity.tags:
                if tag.startswith("deadline-"):
                    parsed = _parse_deadline_tag(tag)
                    if parsed and parsed >= today:
                        days_left = (parsed - today).days
                        deadlines.append(DeadlineItem(
                            project=entity.name,
                            description=entity.context[:80] if entity.context else entity.name,
                            date=parsed,
                            days_left=days_left,
                        ))
                        break  # one deadline per entity

        # From entity context via regex
        for entity in all_entities:
            if entity.context:
                for ds in extract_deadlines(entity.context):
                    parsed = _try_parse_date_string(ds)
                    if parsed and parsed >= today:
                        # Avoid duplicating what we already captured from tags
                        already = any(d.project == entity.name for d in deadlines)
                        if not already:
                            days_left = (parsed - today).days
                            deadlines.append(DeadlineItem(
                                project=entity.name,
                                description=ds,
                                date=parsed,
                                days_left=days_left,
                            ))
                        break

        # From action item deadlines (skip if linked entities are all archived/completed)
        for interaction, actions in pending_pairs:
            linked_ids = {m.entity_id for m in interaction.entities}
            if linked_ids and not linked_ids.intersection(active_entity_ids):
                continue
            for action in actions:
                if action.deadline and action.deadline >= today:
                    days_left = (action.deadline - today).days
                    # entity name from interaction
                    proj_name = (
                        interaction.entities[0].entity_name
                        if interaction.entities
                        else "Action"
                    )
                    deadlines.append(DeadlineItem(
                        project=proj_name,
                        description=action.description[:80],
                        date=action.deadline,
                        days_left=days_left,
                    ))

        deadlines.sort(key=lambda d: d.date)

        # --- Pending actions ---
        pending_actions: list[ActionSummary] = []
        for interaction, actions in pending_pairs:
            # Skip if ALL linked entities are archived/completed
            linked_ids = {m.entity_id for m in interaction.entities}
            if linked_ids and not linked_ids.intersection(active_entity_ids):
                continue

            for action in actions:
                if action.status == "pending":
                    created = (
                        action.created_at.date()
                        if isinstance(action.created_at, datetime)
                        else today
                    )
                    age_days = (today - created).days
                    pending_actions.append(ActionSummary(
                        id=action.id,
                        description=action.description,
                        created_at=created,
                        deadline=action.deadline,
                        age_days=age_days,
                    ))

        # Sort oldest first
        pending_actions.sort(key=lambda a: a.age_days, reverse=True)

        # --- Stale threads ---
        stale_threads: list[StaleItem] = []
        for entity in stale_entities:
            last = (
                entity.last_interaction_at.date()
                if isinstance(entity.last_interaction_at, datetime)
                else today
            )
            days_stale = (today - last).days
            stale_threads.append(StaleItem(
                entity_name=entity.name,
                last_activity=last,
                days_stale=days_stale,
                note=entity.context[:60] if entity.context else "",
            ))

        stale_threads.sort(key=lambda s: s.days_stale, reverse=True)

        # --- Team pulse ---
        # Find person-type entities mentioned in recent interactions
        person_ids = {e.id for e in all_entities if e.type in ("person", "client")}
        team_pulse: list[str] = []
        seen_pulse: set[str] = set()

        for interaction in sorted(recent_interactions, key=lambda i: i.timestamp, reverse=True):
            for mention in interaction.entities:
                if mention.entity_id in person_ids and mention.entity_name not in seen_pulse:
                    seen_pulse.add(mention.entity_name)
                    pulse_line = f"{mention.entity_name} ({mention.role}): {interaction.summary[:80]}"
                    team_pulse.append(pulse_line)
            if len(team_pulse) >= 10:
                break

        # --- Recent decisions ---
        cutoff_30d = datetime.utcnow() - timedelta(days=30)
        recent_decisions: list[DecisionSummary] = []
        for entity in all_entities:
            if entity.type == "decision" and _naive_utc(entity.updated_at) >= cutoff_30d:
                recent_decisions.append(DecisionSummary(
                    name=entity.name,
                    summary=entity.context[:120] if entity.context else entity.name,
                    date=entity.updated_at.date(),
                ))

        recent_decisions.sort(key=lambda d: d.date, reverse=True)

        return WorldStateSections(
            urgent=urgent,
            projects_work=projects_work,
            projects_personal=projects_personal,
            projects_personal=projects_personal,
            waiting_on=waiting_on,
            deadlines=deadlines,
            pending_actions=pending_actions,
            stale_threads=stale_threads,
            team_pulse=team_pulse,
            recent_decisions=recent_decisions,
        )

    # ------------------------------------------------------------------
    # Explorer data computation
    # ------------------------------------------------------------------

    async def _compute_explorer_data(
        self,
        all_entities: list,
        all_interactions: list,
        pending_pairs: list,
    ) -> ExplorerData:
        """Build the rich explorer data for the TUI world explorer."""
        from datetime import timedelta
        today = date.today()

        # Active entity IDs — same filter as _compute_sections
        active_entity_ids = {e.id for e in all_entities if e.status not in ("completed", "archived")}

        # Index interactions by entity_id for fast lookup
        interactions_by_entity: dict[str, list] = {}
        for interaction in all_interactions:
            for mention in interaction.entities:
                interactions_by_entity.setdefault(mention.entity_id, []).append(interaction)

        # Index entities by id
        entity_by_id = {e.id: e for e in all_entities}

        def _make_explorer_interaction(inter) -> ExplorerInteraction:
            return ExplorerInteraction(
                timestamp=inter.timestamp.isoformat() if inter.timestamp else "",
                source=inter.source,
                summary=inter.summary[:120] if inter.summary else "",
                entity_names=[m.entity_name for m in inter.entities],
            )

        # --- Projects ---
        # Pre-index: tasks by project ID (avoids O(n²) in project loop)
        tasks_by_project_id: dict[str, list] = {}
        for e in all_entities:
            if e.type == "task" and e.status in ("active", "paused"):
                for rel in e.relations:
                    if rel.type == "belongs_to" and rel.target_id:
                        tasks_by_project_id.setdefault(rel.target_id, []).append(e)
                        break

        projects: list[ExplorerProjectCard] = []
        for entity in all_entities:
            if entity.type != "project" or entity.status in ("archived", "completed"):
                continue
            # Deadline from tags
            proj_deadline = None
            for tag in entity.tags:
                if tag.startswith("deadline-"):
                    parsed = _parse_deadline_tag(tag)
                    if parsed and (proj_deadline is None or parsed < proj_deadline):
                        proj_deadline = parsed

            blockers = [r.target_name for r in entity.relations if r.type == "blocked_by"]
            key_people = [r.target_name for r in entity.relations if r.type in ("team_member", "works_on", "reports_to")]

            # Pending actions for this project
            proj_actions = []
            for interaction, actions in pending_pairs:
                for mention in interaction.entities:
                    if mention.entity_id == entity.id:
                        for a in actions:
                            if a.status == "pending":
                                proj_actions.append(a.description[:80])
                        break

            # Task entities linked to this project (from pre-built index)
            proj_tasks = []
            for task_entity in tasks_by_project_id.get(entity.id, []):
                priority_marker = {"critical": "!!!", "high": "!!", "medium": "!", "low": ""}.get(task_entity.priority, "")
                proj_tasks.append(f"{priority_marker} {task_entity.name}".strip())

            # Recent interactions (last 5)
            entity_interactions = interactions_by_entity.get(entity.id, [])
            entity_interactions.sort(key=lambda i: i.timestamp, reverse=True)
            recent = [_make_explorer_interaction(i) for i in entity_interactions[:5]]

            projects.append(ExplorerProjectCard(
                entity_id=entity.id,
                name=entity.name,
                scope=entity.scope,
                status=entity.status,
                context=entity.context[:200] if entity.context else "",
                deadline=str(proj_deadline) if proj_deadline else None,
                blockers=blockers,
                key_people=key_people,
                pending_actions=proj_actions,
                tasks=proj_tasks,
                recent_interactions=recent,
            ))

        # Sort: active first, then by name
        projects.sort(key=lambda p: (0 if p.status == "active" else 1, p.name.lower()))

        # Pre-index: person_id → project names (avoids O(n²) in people loop)
        projects_by_person_id: dict[str, list[str]] = {}
        for e in all_entities:
            if e.type == "project":
                for rel in e.relations:
                    if rel.target_id and rel.type in ("works_on", "team_member", "assigned_to", "freelance_client", "client_of"):
                        projects_by_person_id.setdefault(rel.target_id, []).append(e.name)

        # --- People ---
        people: list[ExplorerPersonCard] = []
        for entity in all_entities:
            if entity.type not in ("person", "client") or entity.status in ("archived", "completed"):
                continue

            # Find projects this person is linked to (from pre-built index)
            person_projects = projects_by_person_id.get(entity.id, [])

            # Role from relations or type
            role = entity.type
            for rel in entity.relations:
                if rel.type in ("reports_to", "client_of", "freelance_client"):
                    role = rel.type.replace("_", " ")
                    break

            # Last contact
            entity_interactions = interactions_by_entity.get(entity.id, [])
            entity_interactions.sort(key=lambda i: i.timestamp, reverse=True)
            last_contact = None
            if entity_interactions:
                ts = entity_interactions[0].timestamp
                last_contact = str(ts.date()) if isinstance(ts, datetime) else None

            recent = [_make_explorer_interaction(i) for i in entity_interactions[:5]]

            people.append(ExplorerPersonCard(
                entity_id=entity.id,
                name=entity.name,
                role=role,
                scope=entity.scope,
                context=entity.context[:200] if entity.context else "",
                last_contact=last_contact,
                projects=person_projects,
                recent_interactions=recent,
            ))

        people.sort(key=lambda p: (p.last_contact or "",), reverse=True)

        # --- Decisions ---
        decisions: list[ExplorerDecisionCard] = []
        for entity in all_entities:
            if entity.type != "decision" or entity.status in ("archived", "completed"):
                continue
            related = [r.target_name for r in entity.relations]
            decisions.append(ExplorerDecisionCard(
                entity_id=entity.id,
                name=entity.name,
                summary=entity.context[:200] if entity.context else "",
                date=str(entity.updated_at.date()) if isinstance(entity.updated_at, datetime) else None,
                related_entities=related,
            ))
        decisions.sort(key=lambda d: (d.date or "",), reverse=True)

        # --- Actions (unified) ---
        actions: list[ExplorerActionCard] = []
        for interaction, action_items in pending_pairs:
            # Skip if all linked entities are archived/completed
            linked_ids = {m.entity_id for m in interaction.entities}
            if linked_ids and not linked_ids.intersection(active_entity_ids):
                continue
            proj_name = interaction.entities[0].entity_name if interaction.entities else ""
            for a in action_items:
                created = a.created_at.date() if isinstance(a.created_at, datetime) else today
                age_days = (today - created).days
                actions.append(ExplorerActionCard(
                    id=a.id,
                    description=a.description,
                    status=a.status,
                    project=proj_name,
                    deadline=str(a.deadline) if a.deadline else None,
                    age_days=age_days,
                ))
        # Also include task entities (skip completed/archived)
        for entity in all_entities:
            if entity.type != "task" or entity.status in ("completed", "archived"):
                continue
            created = entity.created_at.date() if isinstance(entity.created_at, datetime) else today
            age_days = (today - created).days

            # Find project from belongs_to
            task_project = ""
            for rel in entity.relations:
                if rel.type == "belongs_to":
                    task_project = rel.target_name
                    break

            # Find deadline from tags
            task_deadline = None
            for tag in entity.tags:
                if tag.startswith("deadline-"):
                    task_deadline = tag[len("deadline-"):]
                    break

            # Map entity status to action-item-style status for consistent rendering
            action_status = {"active": "pending", "paused": "in_progress", "completed": "done"}.get(entity.status, entity.status)

            actions.append(ExplorerActionCard(
                id=entity.id,
                description=entity.name,
                status=action_status,
                project=task_project,
                deadline=task_deadline,
                age_days=age_days,
            ))

        actions.sort(key=lambda a: (0 if a.status == "pending" else 1, -a.age_days))

        # --- Timeline (last 50 interactions, reverse chronological) ---
        sorted_interactions = sorted(all_interactions, key=lambda i: i.timestamp, reverse=True)[:50]
        timeline: list[ExplorerTimelineEntry] = []
        for inter in sorted_interactions:
            timeline.append(ExplorerTimelineEntry(
                timestamp=inter.timestamp.isoformat() if inter.timestamp else "",
                source=inter.source,
                type=inter.type,
                summary=inter.summary[:120] if inter.summary else "",
                entity_names=[m.entity_name for m in inter.entities],
            ))

        # --- Payments / Finance ---
        import calendar
        payments: list[ExplorerPaymentCard] = []
        finance = FinanceSummary()
        current_month = today.month
        current_year = today.year

        # Collect all payment data for forecast computation
        salary_amount = 0
        salary_currency = "INR"
        # forecast_buckets: (year, month) → freelance amount
        forecast_buckets: dict[tuple[int, int], int] = {}
        forecast_received: dict[tuple[int, int], int] = {}

        for entity in all_entities:
            if entity.type != "payment":
                continue

            # Parse structured context
            parsed: dict[str, str] = {}
            for part in entity.context.split("|"):
                part = part.strip()
                if ":" in part:
                    k, v = part.split(":", 1)
                    parsed[k.strip().lower()] = v.strip()

            amount = int(parsed.get("amount", "0") or "0")
            currency = parsed.get("currency", "INR")
            due_str = parsed.get("due", "")
            paid_str = parsed.get("paid", "")
            label = parsed.get("label", "")
            frequency = parsed.get("frequency", "")
            is_salary = "salary" in entity.tags
            is_recurring = "recurring" in entity.tags
            is_target = "target" in entity.tags

            # Detect freelance target entity — not a real payment, just a goal
            if is_target and not is_salary:
                finance.freelance_target = amount
                continue  # Don't add to payments list or forecast

            # Parse dates
            due_date = None
            paid_date = None
            try:
                if due_str:
                    due_date = date.fromisoformat(due_str)
            except ValueError:
                pass
            try:
                if paid_str:
                    paid_date = date.fromisoformat(paid_str)
            except ValueError:
                pass

            is_paid = entity.status == "completed" or paid_date is not None
            is_overdue = not is_paid and due_date is not None and due_date < today
            days_left = (due_date - today).days if due_date and not is_paid else None

            # Detect "owed to you" — personal debts with no due date or "Owed by" label
            is_owed = (
                not is_salary
                and not is_paid
                and (label.lower().startswith("owed by") or (entity.scope == "personal" and not due_date))
            )

            # Find linked project
            project_name = ""
            for rel in entity.relations:
                if rel.type == "belongs_to":
                    project_name = rel.target_name
                    break

            if is_paid:
                status = "paid"
            elif is_owed:
                status = "owed"
            elif is_overdue:
                status = "overdue"
            else:
                status = "pending"

            payments.append(ExplorerPaymentCard(
                entity_id=entity.id,
                name=entity.name,
                project=project_name,
                scope=entity.scope,
                amount=amount,
                currency=currency,
                due_date=due_str or None,
                paid_date=paid_str or None,
                status=status,
                label=label,
                is_salary=is_salary,
                is_recurring=is_recurring,
                days_left=days_left,
            ))

            # Finance summary computation
            if is_salary:
                salary_amount = amount
                salary_currency = currency
                finance.salary_amount = amount
                finance.salary_source = entity.name.replace("Salary - ", "")
                finance.currency = currency
                finance.yearly_received += amount * current_month
                finance.monthly_received += amount
                finance.monthly_expected += amount
            elif is_owed:
                finance.monthly_owed += amount
            elif is_paid:
                finance.yearly_received += amount
                if paid_date and paid_date.month == current_month and paid_date.year == current_year:
                    finance.monthly_received += amount
                    finance.monthly_expected += amount
                # Track in forecast bucket
                if paid_date:
                    key = (paid_date.year, paid_date.month)
                    forecast_received[key] = forecast_received.get(key, 0) + amount
            elif is_overdue:
                finance.monthly_overdue += amount
                finance.monthly_expected += amount
            elif due_date and due_date.month == current_month and due_date.year == current_year:
                finance.monthly_pending += amount
                finance.monthly_expected += amount

            # Bucket into forecast months (non-salary, non-paid, includes owed with due dates)
            if not is_salary and not is_paid and due_date:
                key = (due_date.year, due_date.month)
                forecast_buckets[key] = forecast_buckets.get(key, 0) + amount

            # Yearly expected
            if not is_salary:
                if due_date and due_date.year == current_year:
                    finance.yearly_expected += amount
                elif is_paid and paid_date and paid_date.year == current_year:
                    pass  # already counted
                elif not due_date and not is_paid:
                    finance.yearly_expected += amount

        finance.yearly_expected += salary_amount * 12

        # --- Build monthly forecast ---
        # Financial year: April to March. Only show Apr onwards.
        forecast_months = set()
        fy_start = (current_year, 4)  # April
        fy_end = (current_year + 1, 3)  # March next year

        # Add all months with actual data (only if within financial year)
        for key in forecast_buckets:
            if fy_start <= key <= fy_end:
                forecast_months.add(key)
        for key in forecast_received:
            if fy_start <= key <= fy_end:
                forecast_months.add(key)

        # If freelance target set, show full financial year (Apr - Mar)
        if finance.freelance_target > 0:
            for m in range(4, 13):
                forecast_months.add((current_year, m))
            for m in range(1, 4):
                forecast_months.add((current_year + 1, m))
            # Add target to yearly expected for months without actual entries
            remaining_months = 12 - current_month
            finance.yearly_expected += finance.freelance_target * remaining_months
        else:
            for offset in range(1, 3):
                m = current_month + offset
                y = current_year
                if m > 12:
                    m -= 12
                    y += 1
                forecast_months.add((y, m))

        month_names = ["", "Jan", "Feb", "Mar", "Apr", "May", "Jun",
                       "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]

        # Compute actual freelance per month (from real payments only, not targets)
        actual_freelance_by_month: dict[tuple[int, int], int] = {}
        for entity in all_entities:
            if entity.type != "payment" or "salary" in entity.tags or "target" in entity.tags:
                continue
            parsed_ctx: dict[str, str] = {}
            for part in entity.context.split("|"):
                part = part.strip()
                if ":" in part:
                    k, v = part.split(":", 1)
                    parsed_ctx[k.strip().lower()] = v.strip()
            amt = int(parsed_ctx.get("amount", "0") or "0")
            paid_s = parsed_ctx.get("paid", "")
            due_s = parsed_ctx.get("due", "")
            is_paid = entity.status == "completed"
            # For received: use paid_date month. For pending: use due_date month.
            if is_paid and paid_s:
                try:
                    pd = date.fromisoformat(paid_s)
                    key = (pd.year, pd.month)
                    actual_freelance_by_month[key] = actual_freelance_by_month.get(key, 0) + amt
                except ValueError:
                    pass
            elif due_s:
                try:
                    dd = date.fromisoformat(due_s)
                    key = (dd.year, dd.month)
                    actual_freelance_by_month[key] = actual_freelance_by_month.get(key, 0) + amt
                except ValueError:
                    pass

        # Build forecast with running averages
        # Running average = cumulative freelance / months counted (starting from first month with any freelance)
        cumulative_freelance = 0
        months_with_freelance = 0
        first_freelance_month = None

        for (y, m) in sorted(forecast_months):
            freelance = forecast_buckets.get((y, m), 0)
            received = forecast_received.get((y, m), 0)
            is_future = (y, m) > (current_year, current_month)

            # For future months with no actual entries, use target as expected
            if freelance == 0 and finance.freelance_target > 0 and is_future:
                freelance = finance.freelance_target

            # Current month: add salary as received
            if y == current_year and m == current_month:
                received += salary_amount
            total = salary_amount + freelance

            # Running average tracks actual + projected freelance
            actual_this_month = actual_freelance_by_month.get((y, m), 0)
            if is_future:
                # Future: use target if no actual pipeline, actual pipeline otherwise
                month_contribution = actual_this_month if actual_this_month > 0 else finance.freelance_target
            else:
                month_contribution = actual_this_month

            if month_contribution > 0 or cumulative_freelance > 0:
                cumulative_freelance += month_contribution
                if first_freelance_month is None and month_contribution > 0:
                    first_freelance_month = (y, m)
                if first_freelance_month is not None:
                    months_with_freelance += 1

            running_avg = cumulative_freelance // max(1, months_with_freelance)
            on_target = running_avg >= finance.freelance_target if finance.freelance_target > 0 else True

            # Confirmed freelance = actual pipeline for this month
            freelance_confirmed = actual_freelance_by_month.get((y, m), 0)
            freelance_gap = max(0, finance.freelance_target - freelance_confirmed) if finance.freelance_target > 0 else 0

            finance.forecast.append(MonthForecast(
                label=f"{month_names[m]} {y}",
                year=y,
                month=m,
                salary=salary_amount,
                freelance=freelance,
                freelance_confirmed=freelance_confirmed,
                freelance_gap=freelance_gap,
                total=total,
                received=received,
                freelance_cumulative=cumulative_freelance,
                freelance_running_avg=running_avg,
                on_target=on_target,
            ))

        # ── Professional target tracking (Apr 2026 - Mar 2027) ──
        # Target year starts April, so month 1 = April
        target_start_month = 4  # April
        target_start_year = current_year

        if current_month >= target_start_month:
            months_elapsed = current_month - target_start_month + 1
        else:
            months_elapsed = 0  # Before April, target hasn't started yet

        months_total = 12
        months_remaining = max(0, months_total - months_elapsed)

        # Earned = paid freelance payments (non-salary, non-target)
        freelance_earned = 0
        for p in payments:
            if not p.is_salary and p.status == "paid":
                freelance_earned += p.amount

        # Pipeline = confirmed but not yet paid (non-salary, non-owed, active)
        freelance_pipeline_total = 0
        for p in payments:
            if not p.is_salary and p.status == "pending":
                freelance_pipeline_total += p.amount

        # Owed also counts as "locked in" (money coming)
        owed_for_target = 0
        for p in payments:
            if p.status == "owed":
                owed_for_target += p.amount

        freelance_locked = freelance_earned + freelance_pipeline_total + owed_for_target

        # Cumulative target = months_elapsed × monthly_target
        target_cumulative = finance.freelance_target * months_elapsed
        target_yearly = finance.freelance_target * months_total

        # Ahead/behind
        ahead_behind = freelance_locked - target_cumulative

        # Runway: if ahead, how many months the surplus covers
        runway = ahead_behind / max(1, finance.freelance_target) if ahead_behind > 0 else 0.0

        # Required run rate: what you need per remaining month
        remaining_target = max(0, target_yearly - freelance_locked)
        required_rate = remaining_target // max(1, months_remaining) if months_remaining > 0 else 0

        # Completion %
        completion_pct = (freelance_locked / max(1, target_yearly)) * 100 if target_yearly > 0 else 0

        # Average of what's been earned + pipeline per month elapsed
        freelance_avg = freelance_locked // max(1, months_elapsed) if months_elapsed > 0 else 0

        finance.freelance_target_yearly = target_yearly
        finance.freelance_earned = freelance_earned
        finance.freelance_pipeline = freelance_pipeline_total
        finance.freelance_locked = freelance_locked
        finance.freelance_target_cumulative = target_cumulative
        finance.freelance_ahead_behind = ahead_behind
        finance.freelance_runway_months = round(runway, 1)
        finance.freelance_required_rate = required_rate
        finance.freelance_completion_pct = round(completion_pct, 1)
        finance.freelance_months_elapsed = months_elapsed
        finance.freelance_months_remaining = months_remaining
        finance.freelance_ytd = freelance_locked
        finance.freelance_avg = freelance_avg
        finance.freelance_months_counted = months_elapsed

        # Sort payments: salary first, then overdue, owed, pending, paid
        payments.sort(key=lambda p: (
            0 if p.is_salary else (1 if p.status == "overdue" else (2 if p.status == "owed" else (3 if p.status == "pending" else 4))),
            p.due_date or "9999-12-31",
        ))

        return ExplorerData(
            projects=projects,
            people=people,
            decisions=decisions,
            actions=actions,
            timeline=timeline,
            payments=payments,
            finance_summary=finance,
        )

    # ------------------------------------------------------------------
    # Markdown rendering
    # ------------------------------------------------------------------

    def _render_markdown(self, sections: WorldStateSections, now: datetime) -> str:
        lines: list[str] = []
        date_str = now.strftime("%Y-%m-%d %H:%M UTC")

        lines.append(f"# Current State — {date_str}")
        lines.append("")

        # Urgent
        lines.append("## Urgent")
        if sections.urgent:
            for item in sections.urgent:
                deadline_str = f" (deadline: {item.deadline})" if item.deadline else ""
                lines.append(f"- {item.description}{deadline_str}")
        else:
            lines.append("_Nothing urgent._")
        lines.append("")

        # Projects — Work
        lines.append("## Projects — Work")
        if sections.projects_work:
            for proj in sections.projects_work:
                lines.append(f"### {proj.name}")
                lines.append(proj.status_line)
                if proj.deadline:
                    lines.append(f"Deadline: {proj.deadline}")
                if proj.blockers:
                    lines.append(f"Blockers: {', '.join(proj.blockers)}")
                if proj.key_people:
                    lines.append(f"Team: {', '.join(proj.key_people)}")
                lines.append("")
        else:
            lines.append("_No active Work projects._")
            lines.append("")

        # Projects — Personal
        lines.append("## Projects — Personal (Freelance)")
        if sections.projects_personal:
            for proj in sections.projects_personal:
                lines.append(f"### {proj.name}")
                lines.append(proj.status_line)
                if proj.deadline:
                    lines.append(f"Deadline: {proj.deadline}")
                if proj.blockers:
                    lines.append(f"Blockers: {', '.join(proj.blockers)}")
                if proj.key_people:
                    lines.append(f"Team: {', '.join(proj.key_people)}")
                lines.append("")
        else:
            lines.append("_No active Personal projects._")
            lines.append("")

        # Projects — Personal
        lines.append("## Projects — Personal")
        if sections.projects_personal:
            for proj in sections.projects_personal:
                lines.append(f"### {proj.name}")
                lines.append(proj.status_line)
                if proj.deadline:
                    lines.append(f"Deadline: {proj.deadline}")
                if proj.blockers:
                    lines.append(f"Blockers: {', '.join(proj.blockers)}")
                if proj.key_people:
                    lines.append(f"Team: {', '.join(proj.key_people)}")
                lines.append("")
        else:
            lines.append("_No active personal projects._")
            lines.append("")

        # Waiting On
        lines.append("## Waiting On")
        if sections.waiting_on:
            for item in sections.waiting_on:
                lines.append(
                    f"- {item.description} — from {item.from_person}, {item.days_waiting} days"
                )
        else:
            lines.append("_Not waiting on anyone._")
        lines.append("")

        # Upcoming Deadlines
        lines.append("## Upcoming Deadlines")
        if sections.deadlines:
            for item in sections.deadlines:
                lines.append(
                    f"- {item.project}: {item.description} — {item.days_left} days left"
                )
        else:
            lines.append("_No upcoming deadlines._")
        lines.append("")

        # Pending Actions
        lines.append(f"## Pending Actions ({len(sections.pending_actions)})")
        if sections.pending_actions:
            for action in sections.pending_actions:
                deadline_str = f", due {action.deadline}" if action.deadline else ""
                lines.append(f"- {action.description} (age: {action.age_days}d{deadline_str})")
        else:
            lines.append("_No pending actions._")
        lines.append("")

        # Stale Threads
        lines.append("## Stale Threads")
        if sections.stale_threads:
            for item in sections.stale_threads:
                note = f" — {item.note}" if item.note else ""
                lines.append(
                    f"- {item.entity_name}: last activity {item.days_stale} days ago{note}"
                )
        else:
            lines.append("_No stale threads._")
        lines.append("")

        # Team Pulse
        lines.append("## Team Pulse")
        if sections.team_pulse:
            for pulse_line in sections.team_pulse:
                lines.append(f"- {pulse_line}")
        else:
            lines.append("_No recent team activity._")
        lines.append("")

        # Recent Decisions
        lines.append("## Recent Decisions")
        if sections.recent_decisions:
            for decision in sections.recent_decisions:
                lines.append(f"- {decision.name}: {decision.summary}")
        else:
            lines.append("_No recent decisions._")
        lines.append("")

        return "\n".join(lines)
