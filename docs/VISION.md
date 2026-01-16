# Vision

> Glance: A fast, terminal-native, LLM-first database companion.

---

## The Problem

**DBeaver** and similar tools are:

- Slow and resource-heavy
- Overwhelming with features most users never touch
- Designed for a pre-AI era
- Visually dated

Developers spend too much time writing boilerplate SQL, navigating complex UIs, and context-switching between tools.

---

## The Solution

**Glance** is the anti-DBeaver:

- **Fast** — Rust-native, minimal footprint, instant startup
- **Focused** — Do fewer things, do them well
- **LLM-first** — Natural language is the primary interaction mode
- **Terminal-native** — TUI interface, keyboard-driven, works over SSH

---

## Core Interaction Model

```
┌─────────────────────────────────────────────────────────────┐
│ Glance                                    [db: myapp_prod]  │
├─────────────────────────────────────────┬───────────────────┤
│                                         │ Query Log         │
│  You: Show me users who signed up       │                   │
│       this week but haven't made        │ ▸ SELECT id, em.. │
│       a purchase yet                    │   ✓ 0.023s, 47    │
│                                         │                   │
│  Glance: Found 47 users who signed      │ ▸ SELECT COUNT(*) │
│  up in the last 7 days with no          │   ✓ 0.008s, 1     │
│  orders:                                │                   │
│                                         │                   │
│  ┌─────────────────────────────────┐    │                   │
│  │ id │ email          │ created  │    │                   │
│  ├────┼────────────────┼──────────┤    │                   │
│  │ 42 │ alice@mail.com │ Jan 14   │    │                   │
│  │ 43 │ bob@test.io    │ Jan 14   │    │                   │
│  │ .. │ ...            │ ...      │    │                   │
│  └─────────────────────────────────┘    │                   │
│                                         │                   │
├─────────────────────────────────────────┴───────────────────┤
│ > _                                                         │
└─────────────────────────────────────────────────────────────┘
```

**Key behaviors:**

- LLM auto-discovers schema on connect
- Non-destructive queries (SELECT) execute automatically
- Mutating queries (INSERT/UPDATE/DELETE) require confirmation
- Query sidebar shows SQL for transparency and learning
- Results are paginated and scrollable

---

## Long-Term Vision

Glance becomes the **AI-powered database companion** for developers:

1. **Query & Explore** (MVP) — Chat with your data
2. **Schema Design** — "Help me design a schema for a blog with comments"
3. **Migration Assistance** — "Generate a migration to add soft deletes"
4. **Performance Analysis** — "Why is this query slow?"
5. **Data Quality** — "Find orphaned records" / "Check referential integrity"
6. **Documentation** — "Document this schema" / "Explain this table"

The unifying principle: **Any LLM work you want to do on databases, in one fast tool.**

---

## Target Users

1. **Backend developers** — Daily database work, want speed over features
2. **Data-curious developers** — Know enough SQL to be dangerous, want AI help
3. **DevOps/SREs** — Quick database checks over SSH

---

## Non-Goals (for now)

- Replacing full-featured database IDEs
- Visual query builders
- ER diagram generation
- Database administration (user management, backups)
- Supporting every database under the sun

---

## Competitive Positioning

| Tool       | Speed | LLM | Terminal | Open Source |
| ---------- | ----- | --- | -------- | ----------- |
| DBeaver    | ❌    | ❌  | ❌       | ✅          |
| DataGrip   | ⚠️    | ⚠️  | ❌       | ❌          |
| Vanna      | ⚠️    | ✅  | ❌       | ✅          |
| Gobang     | ✅    | ❌  | ✅       | ✅          |
| **Glance** | ✅    | ✅  | ✅       | ✅          |
