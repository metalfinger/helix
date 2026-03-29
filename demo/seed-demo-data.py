"""
seed-demo-data.py — Demo data seeder for Helix Memory System
=============================================================
This script populates a local Qdrant instance with realistic fake entities
to showcase the Helix AI session dashboard + memory system.

All data here is fictional. Names, projects, and interactions are made up
for demonstration purposes.

Usage:
    python seed-demo-data.py          # Seed demo data
    python seed-demo-data.py --clean  # Wipe collection and reseed

Requirements:
    pip install qdrant-client
"""

import argparse
import sys
import uuid
import json
import random
from datetime import datetime, timedelta

try:
    from qdrant_client import QdrantClient
    from qdrant_client.models import (
        Distance,
        VectorParams,
        PointStruct,
    )
except ImportError:
    print("\n  [!] qdrant-client not installed. Run: pip install qdrant-client\n")
    sys.exit(1)

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

QDRANT_HOST = "localhost"
QDRANT_PORT = 6333
COLLECTION = "helix_entities"
VECTOR_DIM = 384

NOW = datetime.now()


def days_ago(n):
    return (NOW - timedelta(days=n)).isoformat()


def days_from_now(n):
    return (NOW + timedelta(days=n)).isoformat()


def random_vector():
    return [random.gauss(0, 1) for _ in range(VECTOR_DIM)]


# ---------------------------------------------------------------------------
# Demo Data
# ---------------------------------------------------------------------------

PROJECTS = [
    {
        "name": "ChaiOS",
        "type": "project",
        "description": "Cross-platform mobile app for chai delivery tracking. React Native + Node.js backend.",
        "scope": "work",
        "status": "active",
        "priority": "high",
        "created_at": days_ago(45),
    },
    {
        "name": "Netra",
        "type": "project",
        "description": "Computer vision pipeline for retail analytics. YOLOv8 + custom tracking.",
        "scope": "work",
        "status": "active",
        "priority": "critical",
        "created_at": days_ago(60),
    },
    {
        "name": "Jugaad Board",
        "type": "project",
        "description": "Custom mechanical keyboard firmware. QMK fork with macro layers.",
        "scope": "personal",
        "status": "active",
        "priority": "medium",
        "created_at": days_ago(30),
    },
    {
        "name": "Desi Beats",
        "type": "project",
        "description": "AI-powered tabla rhythm generator. Diffusion model trained on classical recordings.",
        "scope": "personal",
        "status": "planning",
        "priority": "low",
        "created_at": days_ago(10),
    },
]

PEOPLE = [
    {
        "name": "Arjun Mehta",
        "type": "person",
        "description": "Tech lead, drives architecture decisions on ChaiOS. Opinionated about microservices.",
        "role": "colleague",
        "scope": "work",
        "projects": ["ChaiOS"],
    },
    {
        "name": "Priya Sharma",
        "type": "person",
        "description": "ML engineer on Netra. PhD dropout, knows more about transformers than anyone.",
        "role": "colleague",
        "scope": "work",
        "projects": ["Netra"],
    },
    {
        "name": "Vikram Desai",
        "type": "person",
        "description": "DevOps. Manages all infra. Will mass-ping Slack if you push to prod on Friday.",
        "role": "colleague",
        "scope": "work",
        "projects": ["ChaiOS", "Netra"],
    },
    {
        "name": "Neha Krishnan",
        "type": "person",
        "description": "Design lead. Figma wizard. Refuses to approve anything without 8px grid alignment.",
        "role": "colleague",
        "scope": "work",
        "projects": ["ChaiOS"],
    },
    {
        "name": "Rohan Patel",
        "type": "person",
        "description": "College friend, co-conspirator on Jugaad Board. Brings soldering iron to coffee meetups.",
        "role": "friend",
        "scope": "personal",
        "projects": ["Jugaad Board"],
    },
]

TASKS = [
    {
        "name": "Migrate ChaiOS auth to JWT",
        "type": "task",
        "description": "Replace session-based auth with JWT tokens. Arjun wants RS256 signing.",
        "project": "ChaiOS",
        "deadline": days_from_now(2),
        "priority": "high",
        "status": "in_progress",
        "assigned_to": "self",
    },
    {
        "name": "Review Priya's YOLO fine-tuning PR",
        "type": "task",
        "description": "PR #247 — custom anchor boxes for retail shelf detection. Needs perf benchmarks.",
        "project": "Netra",
        "deadline": days_from_now(1),
        "priority": "critical",
        "status": "pending",
        "assigned_to": "self",
    },
    {
        "name": "Fix payment gateway timeout on UPI callbacks",
        "type": "task",
        "description": "UPI callback webhook times out after 30s. Razorpay support says increase to 60s, but we need to fix the actual bottleneck.",
        "project": "ChaiOS",
        "deadline": days_from_now(3),
        "priority": "high",
        "status": "pending",
        "assigned_to": "self",
    },
    {
        "name": "Order Cherry MX switches from Meckeys",
        "type": "task",
        "description": "Cherry MX Brown x70. Check if Meckeys has the RGB variant in stock.",
        "project": "Jugaad Board",
        "deadline": days_from_now(7),
        "priority": "low",
        "status": "pending",
        "assigned_to": "self",
    },
    {
        "name": "Set up Grafana dashboards for Netra inference latency",
        "type": "task",
        "description": "P95/P99 latency panels, GPU utilization, batch throughput. Vikram needs to provision the monitoring namespace first.",
        "project": "Netra",
        "deadline": days_from_now(4),
        "priority": "medium",
        "status": "blocked",
        "blocked_by": "Vikram hasn't provisioned the monitoring namespace",
        "assigned_to": "self",
    },
    {
        "name": "Write tabla sample preprocessing script",
        "type": "task",
        "description": "Normalize audio samples, segment individual strokes (na, tin, dha, ge), export as WAV clips.",
        "project": "Desi Beats",
        "deadline": None,
        "priority": "low",
        "status": "planning",
        "assigned_to": "self",
    },
]

DECISIONS = [
    {
        "name": "Chose Qdrant over Pinecone for vector storage",
        "type": "decision",
        "description": "Self-hosted, no vendor lock-in, Vikram already has Docker Compose templates. Pinecone's free tier limits were too restrictive for our embedding volume.",
        "reason": "Self-hosted, no vendor lock-in, Vikram already has Docker Compose templates",
        "date": days_ago(14),
        "projects": ["Netra", "ChaiOS"],
    },
    {
        "name": "Switched ChaiOS from REST to tRPC",
        "type": "decision",
        "description": "Arjun insisted. He was right — type safety across the stack is worth the migration pain. End-to-end types from Prisma to React Native.",
        "reason": "Type safety across the stack is worth the migration pain",
        "date": days_ago(7),
        "projects": ["ChaiOS"],
    },
    {
        "name": "Rejected microservices for Netra pipeline",
        "type": "decision",
        "description": "Priya's models need shared GPU memory. Monolith with clean module boundaries wins here. Revisit if we need to scale inference independently.",
        "reason": "Priya's models need shared GPU memory. Monolith with clean module boundaries wins here.",
        "date": days_ago(3),
        "projects": ["Netra"],
    },
]

INTERACTIONS = [
    {
        "name": "Standup — ChaiOS auth migration behind schedule",
        "type": "interaction",
        "description": "Standup: Arjun flagged ChaiOS auth migration is behind schedule. Vikram offered to help with JWT secret rotation. Agreed to pair on it tomorrow morning.",
        "participants": ["Arjun Mehta", "Vikram Desai"],
        "source": "meeting",
        "date": days_ago(1),
        "action_items": ["Pair with Vikram on JWT secret rotation", "Update migration timeline in Notion"],
    },
    {
        "name": "Netra v2 inference demo",
        "type": "interaction",
        "description": "Priya demoed Netra v2 inference — 40ms per frame on A100. Team impressed. She wants to try TensorRT optimization next, estimates 15ms target.",
        "participants": ["Priya Sharma"],
        "source": "meeting",
        "date": days_ago(2),
        "action_items": ["Benchmark TensorRT vs ONNX runtime", "Schedule GPU allocation with Vikram"],
    },
    {
        "name": "Coffee chat — Jugaad Board PCB sourcing",
        "type": "interaction",
        "description": "Coffee chat with Rohan about Jugaad Board. He found a cheaper PCB manufacturer in Shenzhen — $2.50/unit vs $4.80 from current supplier. MOQ is 50 units though.",
        "participants": ["Rohan Patel"],
        "source": "in_person",
        "date": days_ago(3),
        "action_items": ["Get quote from Shenzhen manufacturer", "Check if 50 MOQ works for first batch"],
    },
    {
        "name": "ChaiOS dashboard redesign rejected",
        "type": "interaction",
        "description": "Neha rejected the ChaiOS dashboard redesign. Quote: 'This looks like it was designed by someone who thinks Comic Sans is a valid font.' Need to redo with proper design tokens.",
        "participants": ["Neha Krishnan"],
        "source": "slack",
        "date": days_ago(4),
        "action_items": ["Redo dashboard with Neha's design tokens", "Set up 1:1 review before next submission"],
    },
]


# ---------------------------------------------------------------------------
# Seeder
# ---------------------------------------------------------------------------

BANNER = r"""
    ╦ ╦╔═╗╦  ╦═╗ ╦
    ╠═╣║╣ ║  ║╔╩╦╝
    ╩ ╩╚═╝╩═╝╩╩ ╚═
    Memory System — Demo Data Seeder
    ──────────────────────────────────
"""


def make_point(entity: dict) -> PointStruct:
    """Convert an entity dict into a Qdrant PointStruct."""
    return PointStruct(
        id=str(uuid.uuid4()),
        vector=random_vector(),
        payload=entity,
    )


def seed(client: QdrantClient, clean: bool = False):
    print(BANNER)

    # ------------------------------------------------------------------
    # Collection setup
    # ------------------------------------------------------------------
    collections = [c.name for c in client.get_collections().collections]

    if clean and COLLECTION in collections:
        print("  [~] Wiping existing collection...")
        client.delete_collection(COLLECTION)
        collections.remove(COLLECTION)

    if COLLECTION not in collections:
        print(f"  [+] Creating collection '{COLLECTION}' (dim={VECTOR_DIM})")
        client.create_collection(
            collection_name=COLLECTION,
            vectors_config=VectorParams(size=VECTOR_DIM, distance=Distance.COSINE),
        )
    else:
        print(f"  [=] Collection '{COLLECTION}' already exists")

    # ------------------------------------------------------------------
    # Seed entities
    # ------------------------------------------------------------------
    all_entities = []

    print("\n  Seeding projects...", end=" ")
    for p in PROJECTS:
        all_entities.append(make_point(p))
    print(f"{len(PROJECTS)} added")

    print("  Seeding people...", end=" ")
    for p in PEOPLE:
        all_entities.append(make_point(p))
    print(f"{len(PEOPLE)} added")

    print("  Seeding tasks...", end=" ")
    for t in TASKS:
        all_entities.append(make_point(t))
    print(f"{len(TASKS)} added")

    print("  Seeding decisions...", end=" ")
    for d in DECISIONS:
        all_entities.append(make_point(d))
    print(f"{len(DECISIONS)} added")

    print("  Seeding interactions...", end=" ")
    for i in INTERACTIONS:
        all_entities.append(make_point(i))
    print(f"{len(INTERACTIONS)} added")

    # ------------------------------------------------------------------
    # Upsert all at once
    # ------------------------------------------------------------------
    print(f"\n  [>] Upserting {len(all_entities)} entities to Qdrant...")
    client.upsert(collection_name=COLLECTION, points=all_entities)

    total = client.count(collection_name=COLLECTION).count
    print(f"  [✓] Done! Collection '{COLLECTION}' now has {total} entities.\n")

    # ------------------------------------------------------------------
    # Summary
    # ------------------------------------------------------------------
    print("  ┌─────────────────────────────────────┐")
    print("  │  Demo Data Summary                   │")
    print("  ├─────────────────────────────────────┤")
    print(f"  │  Projects:     {len(PROJECTS):>3}                   │")
    print(f"  │  People:       {len(PEOPLE):>3}                   │")
    print(f"  │  Tasks:        {len(TASKS):>3}                   │")
    print(f"  │  Decisions:    {len(DECISIONS):>3}                   │")
    print(f"  │  Interactions: {len(INTERACTIONS):>3}                   │")
    print(f"  │  ─────────────────                  │")
    print(f"  │  Total:        {len(all_entities):>3}                   │")
    print("  └─────────────────────────────────────┘")
    print()
    print("  Now run Helix and call helix_get_world_state() to see it in action!")
    print()


def main():
    parser = argparse.ArgumentParser(description="Seed Helix Memory with demo data")
    parser.add_argument(
        "--clean",
        action="store_true",
        help="Wipe the collection and reseed from scratch",
    )
    args = parser.parse_args()

    try:
        client = QdrantClient(host=QDRANT_HOST, port=QDRANT_PORT, timeout=5)
        # Quick connectivity check
        client.get_collections()
    except Exception:
        print(BANNER)
        print("  [!] Could not connect to Qdrant at localhost:6333")
        print()
        print("  Make sure Qdrant is running:")
        print("    docker run -p 6333:6333 qdrant/qdrant")
        print()
        print("  Or with Docker Compose:")
        print("    docker compose up qdrant")
        print()
        sys.exit(1)

    seed(client, clean=args.clean)


if __name__ == "__main__":
    main()
