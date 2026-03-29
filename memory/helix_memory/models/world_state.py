"""
helix_memory.models.world_state

WorldState model: a synthesised snapshot of the user's current context,
derived from recent interactions and entities.
"""

from __future__ import annotations

from datetime import datetime, date
from typing import Optional

from pydantic import BaseModel, Field


class UrgentItem(BaseModel):
    description: str
    source_entity: str = ""
    deadline: Optional[date] = None
    importance: float = 0.9


class ProjectSummary(BaseModel):
    entity_id: str
    name: str
    status_line: str
    deadline: Optional[date] = None
    blockers: list[str] = []
    key_people: list[str] = []


class WaitingItem(BaseModel):
    description: str
    from_person: str
    since: date
    days_waiting: int


class DeadlineItem(BaseModel):
    project: str
    description: str
    date: date
    days_left: int


class ActionSummary(BaseModel):
    id: str
    description: str
    created_at: date
    deadline: Optional[date] = None
    age_days: int


class StaleItem(BaseModel):
    entity_name: str
    last_activity: date
    days_stale: int
    note: str = ""


class DecisionSummary(BaseModel):
    name: str
    summary: str
    date: date


class ExplorerInteraction(BaseModel):
    """Compact interaction for embedding inside explorer cards."""
    timestamp: str = ""
    source: str = ""
    summary: str = ""
    entity_names: list[str] = []

class ExplorerProjectCard(BaseModel):
    entity_id: str = ""
    name: str = ""
    scope: str = ""
    status: str = ""
    context: str = ""
    deadline: Optional[str] = None
    blockers: list[str] = []
    key_people: list[str] = []
    pending_actions: list[str] = []
    tasks: list[str] = []  # task entity names linked via belongs_to
    recent_interactions: list[ExplorerInteraction] = []

class ExplorerPersonCard(BaseModel):
    entity_id: str = ""
    name: str = ""
    role: str = ""
    scope: str = ""
    context: str = ""
    last_contact: Optional[str] = None
    projects: list[str] = []
    recent_interactions: list[ExplorerInteraction] = []

class ExplorerDecisionCard(BaseModel):
    entity_id: str = ""
    name: str = ""
    summary: str = ""
    date: Optional[str] = None
    related_entities: list[str] = []

class ExplorerActionCard(BaseModel):
    id: str = ""
    description: str = ""
    status: str = "pending"
    project: str = ""
    deadline: Optional[str] = None
    age_days: int = 0

class ExplorerTimelineEntry(BaseModel):
    timestamp: str = ""
    source: str = ""
    type: str = ""
    summary: str = ""
    entity_names: list[str] = []

class ExplorerPaymentCard(BaseModel):
    """Financial tracking card for TUI finance overlay."""
    entity_id: str = ""
    name: str = ""
    project: str = ""
    scope: str = ""
    amount: int = 0
    currency: str = "INR"
    due_date: Optional[str] = None
    paid_date: Optional[str] = None
    status: str = "pending"  # pending, paid, overdue
    label: str = ""  # "Advance", "Final", "Monthly salary"
    is_salary: bool = False
    is_recurring: bool = False
    days_left: Optional[int] = None

class MonthForecast(BaseModel):
    """Forecast for a single month."""
    label: str = ""  # "Mar 2026"
    year: int = 0
    month: int = 0
    salary: int = 0
    freelance: int = 0              # Total expected freelance (confirmed + target fill)
    freelance_confirmed: int = 0    # Actual pipeline/payments for this month
    freelance_gap: int = 0          # max(0, target - confirmed) — how much more needed
    total: int = 0
    received: int = 0
    freelance_cumulative: int = 0
    freelance_running_avg: int = 0
    on_target: bool = True

class FinanceSummary(BaseModel):
    """Monthly + yearly financial summary."""
    monthly_received: int = 0
    monthly_pending: int = 0
    monthly_overdue: int = 0
    monthly_owed: int = 0
    monthly_expected: int = 0
    yearly_received: int = 0
    yearly_expected: int = 0
    currency: str = "INR"
    salary_amount: int = 0
    salary_source: str = ""
    freelance_target: int = 0           # Monthly freelance target
    freelance_target_yearly: int = 0     # Annual target (monthly × 12)
    freelance_earned: int = 0            # Actually received (paid)
    freelance_pipeline: int = 0          # Confirmed but not yet paid
    freelance_locked: int = 0            # earned + pipeline (total secured)
    freelance_target_cumulative: int = 0 # Where straight-line says you should be by now
    freelance_ahead_behind: int = 0      # locked - cumulative (positive = ahead)
    freelance_runway_months: float = 0   # surplus / monthly target (months of buffer)
    freelance_required_rate: int = 0     # remaining target / remaining months
    freelance_completion_pct: float = 0  # locked / yearly target × 100
    freelance_months_elapsed: int = 0    # Months since target start (April = 1)
    freelance_months_remaining: int = 0
    freelance_ytd: int = 0
    freelance_avg: int = 0
    freelance_months_counted: int = 0
    forecast: list[MonthForecast] = []

class ExplorerData(BaseModel):
    projects: list[ExplorerProjectCard] = []
    people: list[ExplorerPersonCard] = []
    decisions: list[ExplorerDecisionCard] = []
    actions: list[ExplorerActionCard] = []
    timeline: list[ExplorerTimelineEntry] = []
    payments: list[ExplorerPaymentCard] = []
    finance_summary: FinanceSummary = Field(default_factory=FinanceSummary)


class WorldStateSections(BaseModel):
    urgent: list[UrgentItem] = []
    projects_work: list[ProjectSummary] = []
    projects_personal: list[ProjectSummary] = []
    waiting_on: list[WaitingItem] = []
    deadlines: list[DeadlineItem] = []
    pending_actions: list[ActionSummary] = []
    stale_threads: list[StaleItem] = []
    team_pulse: list[str] = []
    recent_decisions: list[DecisionSummary] = []


class WorldState(BaseModel):
    file_schema_version: int = 1
    version: int
    generated_at: datetime
    stale_after: datetime
    checksum: str = ""
    entity_count: int = 0
    interaction_count: int = 0
    pending_action_count: int = 0
    document: str
    sections: WorldStateSections = Field(default_factory=WorldStateSections)
    generation_duration_ms: int = 0
    explorer: ExplorerData = Field(default_factory=ExplorerData)
