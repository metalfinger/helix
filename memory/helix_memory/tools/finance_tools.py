"""
helix_memory.tools.finance_tools

MCP tool handlers for financial tracking — add payments, set salary,
mark as paid, get financial summary.
"""

from __future__ import annotations

import logging
import re
from datetime import date, datetime
from typing import Optional

from helix_memory.core.embeddings import embed
from helix_memory.models.entity import EntityRelation, WorldEntity

logger = logging.getLogger(__name__)


def _parse_payment_context(context: str) -> dict:
    """Parse structured payment context into a dict.

    Format: "amount: 50000 | currency: INR | due: 2026-04-01 | paid: 2026-03-16 | label: Advance | 1 of 2"
    """
    result = {}
    for part in context.split("|"):
        part = part.strip()
        if ":" in part:
            key, val = part.split(":", 1)
            result[key.strip().lower()] = val.strip()
        elif re.match(r"\d+ of \d+", part):
            result["installment"] = part
    return result


def _build_payment_context(
    amount: int,
    currency: str = "INR",
    due_date: str = "",
    paid_date: str = "",
    label: str = "",
    installment: str = "",
    frequency: str = "",
    pay_day: int = 0,
) -> str:
    """Build structured context string for a payment entity."""
    parts = [f"amount: {amount}", f"currency: {currency}"]
    if due_date:
        parts.append(f"due: {due_date}")
    if paid_date:
        parts.append(f"paid: {paid_date}")
    if label:
        parts.append(f"label: {label}")
    if installment:
        parts.append(installment)
    if frequency:
        parts.append(f"frequency: {frequency}")
    if pay_day:
        parts.append(f"pay_day: {pay_day}")
    return " | ".join(parts)


def _fmt_amount(amount: int, currency: str = "INR") -> str:
    """Format amount with currency symbol."""
    if currency == "INR":
        # Indian number formatting (lakhs/crores)
        if amount >= 10000000:
            return f"Rs {amount / 10000000:.2f} Cr"
        elif amount >= 100000:
            return f"Rs {amount / 100000:.2f} L"
        else:
            return f"Rs {amount:,}"
    elif currency == "USD":
        return f"${amount:,}"
    return f"{currency} {amount:,}"


# ---------------------------------------------------------------------------
# Tool: helix_add_payment
# ---------------------------------------------------------------------------

async def helix_add_payment(
    ctx: dict,
    project: str,
    amount: int,
    due_date: str = "",
    label: str = "",
    currency: str = "INR",
    scope: str = "personal",
    installment: str = "",
) -> dict:
    """Add a payment/invoice linked to a project.

    Args:
        ctx: Tool context.
        project: Project name to link to.
        amount: Payment amount (integer).
        due_date: Due date (YYYY-MM-DD).
        label: Label (e.g. "Advance", "Final", "Milestone 1").
        currency: Currency code (default INR).
        scope: Scope (default personal).
        installment: Installment info (e.g. "1 of 3").
    """
    store = ctx["store"]
    resolver = ctx["resolver"]

    # Resolve project
    project_entity = None
    try:
        project_entity = await resolver.resolve(project)
    except Exception:
        pass

    name_parts = [project]
    if label:
        name_parts.append(label)
    name = " - ".join(name_parts)

    context = _build_payment_context(
        amount=amount,
        currency=currency,
        due_date=due_date,
        label=label,
        installment=installment,
    )

    tags = ["payment", "freelance", f"amount-{amount}"]
    if due_date:
        tags.append(f"deadline-{due_date}")

    relations = []
    if project_entity:
        relations.append(EntityRelation(
            target_id=project_entity.id,
            target_name=project_entity.name,
            type="belongs_to",
        ))

    entity = WorldEntity(
        type="payment",
        name=name,
        scope=scope,
        status="active",
        priority="medium",
        context=context,
        tags=tags,
        relations=relations,
    )
    entity.prepare_for_save()
    vector = embed(entity.embedding_text)
    await store.upsert_entity(entity, vector)

    return {
        "entity_id": entity.id,
        "name": entity.name,
        "amount": _fmt_amount(amount, currency),
        "due_date": due_date or "not set",
        "project": project_entity.name if project_entity else project,
    }


# ---------------------------------------------------------------------------
# Tool: helix_set_salary
# ---------------------------------------------------------------------------

async def helix_set_salary(
    ctx: dict,
    amount: int,
    source: str = "Employer",
    pay_day: int = 1,
    currency: str = "INR",
    scope: str = "work",
) -> dict:
    """Set or update monthly salary.

    Args:
        ctx: Tool context.
        amount: Monthly salary amount.
        source: Employer name (default Employer).
        pay_day: Day of month salary is received (default 1).
        currency: Currency code (default INR).
        scope: Scope (default work).
    """
    store = ctx["store"]
    resolver = ctx["resolver"]

    name = f"Salary - {source}"

    # Check if salary entity already exists
    existing = None
    try:
        existing = await resolver.resolve(name)
    except Exception:
        pass

    context = _build_payment_context(
        amount=amount,
        currency=currency,
        frequency="monthly",
        pay_day=pay_day,
        label="Monthly salary",
    )

    if existing:
        existing.context = context
        existing.tags = ["payment", "salary", "recurring", "monthly", f"amount-{amount}"]
        existing.prepare_for_save()
        vector = embed(existing.embedding_text)
        await store.upsert_entity(existing, vector)
        return {"updated": True, "name": name, "amount": _fmt_amount(amount, currency), "pay_day": pay_day}

    entity = WorldEntity(
        type="payment",
        name=name,
        scope=scope,
        status="active",
        priority="medium",
        context=context,
        tags=["payment", "salary", "recurring", "monthly", f"amount-{amount}"],
    )
    entity.prepare_for_save()
    vector = embed(entity.embedding_text)
    await store.upsert_entity(entity, vector)

    return {"created": True, "name": name, "amount": _fmt_amount(amount, currency), "pay_day": pay_day}


# ---------------------------------------------------------------------------
# Tool: helix_mark_paid
# ---------------------------------------------------------------------------

async def helix_mark_paid(
    ctx: dict,
    name: str,
    paid_date: str = "",
    note: str = "",
) -> dict:
    """Mark a payment as received/paid.

    Args:
        ctx: Tool context.
        name: Payment name (fuzzy matched).
        paid_date: Date payment was received (YYYY-MM-DD). Defaults to today.
        note: Optional note.
    """
    store = ctx["store"]
    resolver = ctx["resolver"]

    entity = await resolver.resolve(name)
    if entity is None:
        return {"error": f"Payment '{name}' not found"}

    if not paid_date:
        paid_date = date.today().isoformat()

    # Parse existing context and update
    parsed = _parse_payment_context(entity.context)
    parsed["paid"] = paid_date
    entity.context = _build_payment_context(
        amount=int(parsed.get("amount", 0)),
        currency=parsed.get("currency", "INR"),
        due_date=parsed.get("due", ""),
        paid_date=paid_date,
        label=parsed.get("label", ""),
        installment=parsed.get("installment", ""),
        frequency=parsed.get("frequency", ""),
        pay_day=int(parsed.get("pay_day", 0)),
    )
    if note:
        entity.context += f" | note: {note}"

    entity.status = "completed"
    entity.prepare_for_save()
    vector = embed(entity.embedding_text)
    await store.upsert_entity(entity, vector)

    return {"marked_paid": True, "name": entity.name, "paid_date": paid_date}


# ---------------------------------------------------------------------------
# Tool: helix_finance_summary
# ---------------------------------------------------------------------------

async def helix_finance_summary(ctx: dict) -> str:
    """Get financial summary — this month's income, upcoming, overdue, YTD.

    Returns formatted markdown.
    """
    store = ctx["store"]

    # Fetch all payment entities
    all_entities = await store.list_all_entities(include_archived=False)
    payments = [e for e in all_entities if e.type == "payment"]

    today = date.today()
    current_month = today.month
    current_year = today.year

    salary_monthly = 0
    salary_source = ""
    this_month_received = 0
    this_month_pending = 0
    overdue = []
    upcoming = []
    ytd_received = 0

    for p in payments:
        parsed = _parse_payment_context(p.context)
        amount = int(parsed.get("amount", 0))
        currency = parsed.get("currency", "INR")
        due_str = parsed.get("due", "")
        paid_str = parsed.get("paid", "")
        frequency = parsed.get("frequency", "")
        label = parsed.get("label", "")

        # Salary
        if "salary" in p.tags:
            salary_monthly = amount
            salary_source = p.name.replace("Salary - ", "")
            # Salary counts as received for every month this year up to current
            ytd_received += salary_monthly * current_month
            this_month_received += salary_monthly
            continue

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

        is_paid = p.status == "completed" or paid_date is not None

        if is_paid:
            ytd_received += amount
            if paid_date and paid_date.month == current_month and paid_date.year == current_year:
                this_month_received += amount
        elif due_date:
            if due_date < today:
                overdue.append((p.name, amount, currency, due_date))
            elif due_date.month == current_month and due_date.year == current_year:
                this_month_pending += amount
                upcoming.append((p.name, amount, currency, due_date))
            else:
                upcoming.append((p.name, amount, currency, due_date))
        else:
            this_month_pending += amount

    # Format output
    lines = ["## Finance Summary\n"]

    if salary_monthly:
        lines.append(f"**Salary:** {_fmt_amount(salary_monthly)} /month ({salary_source}, {today.strftime('%B %Y')})")
        lines.append("")

    lines.append(f"### This Month ({today.strftime('%B %Y')})")
    lines.append(f"- Received: **{_fmt_amount(this_month_received)}**")
    if this_month_pending:
        lines.append(f"- Pending: **{_fmt_amount(this_month_pending)}**")
    lines.append(f"- Expected total: **{_fmt_amount(this_month_received + this_month_pending)}**")
    lines.append("")

    if overdue:
        lines.append("### Overdue")
        for name, amt, cur, due in sorted(overdue, key=lambda x: x[3]):
            days = (today - due).days
            lines.append(f"- **{name}**: {_fmt_amount(amt, cur)} — {days}d overdue (due {due.isoformat()})")
        lines.append("")

    if upcoming:
        lines.append("### Upcoming")
        for name, amt, cur, due in sorted(upcoming, key=lambda x: x[3]):
            days = (due - today).days
            lines.append(f"- **{name}**: {_fmt_amount(amt, cur)} — due {due.isoformat()} ({days}d)")
        lines.append("")

    lines.append("### Year to Date")
    lines.append(f"- Total received: **{_fmt_amount(ytd_received)}**")

    return "\n".join(lines)
