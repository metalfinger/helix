# Sample `helix_get_world_state()` Output

> This is what the world state looks like in Claude Code when Helix Memory is populated with the demo data from `seed-demo-data.py`.

---

```markdown
# 🌍 World State — March 29, 2026

## 🔴 URGENT (2)
- **Review Priya's YOLO fine-tuning PR** — Netra [due tomorrow]
- **Migrate ChaiOS auth to JWT** — ChaiOS [due in 2 days, in progress]

## 📋 Active Projects
### Work
- **ChaiOS** — Cross-platform chai delivery app [3 tasks, 1 blocked]
- **Netra** — CV pipeline for retail analytics [2 tasks, 1 blocked]

### Personal
- **Jugaad Board** — Custom mech keyboard firmware [1 task]
- **Desi Beats** — AI tabla rhythm generator [1 task, planning]

## 📅 Deadlines (next 7 days)
| Task | Project | Due | Status |
|------|---------|-----|--------|
| Review Priya's YOLO PR | Netra | Mar 30 | pending |
| Migrate auth to JWT | ChaiOS | Mar 31 | in_progress |
| Fix UPI callback timeout | ChaiOS | Apr 1 | pending |
| Grafana dashboards | Netra | Apr 2 | blocked |
| Order Cherry MX switches | Jugaad Board | Apr 5 | pending |

## ⏳ Waiting On
- **Vikram Desai** — monitoring namespace provisioning (blocking Grafana dashboards)

## 🚧 Blocked Tasks (1)
- **Grafana dashboards for Netra inference latency** — blocked by: "Vikram hasn't provisioned the monitoring namespace"

## 🕸️ Stale Threads (no activity >5 days)
- Desi Beats — no updates since initial planning

## 💡 Recent Decisions
- Rejected microservices for Netra (3 days ago) — "Priya's models need shared GPU memory. Monolith with clean module boundaries wins here."
- Switched ChaiOS to tRPC (1 week ago) — "Type safety across the stack is worth the migration pain."
- Chose Qdrant over Pinecone (2 weeks ago) — "Self-hosted, no vendor lock-in."

## 💬 Recent Activity
- **Yesterday** — Standup: Arjun flagged auth migration behind schedule. Vikram offered to help with JWT rotation.
- **2 days ago** — Priya demoed Netra v2 inference — 40ms/frame on A100. Team impressed.
- **3 days ago** — Coffee with Rohan: found cheaper PCB manufacturer in Shenzhen ($2.50/unit).
- **4 days ago** — Neha rejected dashboard redesign: "Looks like it was designed by someone who thinks Comic Sans is a valid font."

## 📊 Quick Stats
- 4 active projects | 6 open tasks | 1 blocked | 5 people tracked
- Next deadline: tomorrow (Review Priya's YOLO PR)
- Most active project: ChaiOS (3 tasks)
```
