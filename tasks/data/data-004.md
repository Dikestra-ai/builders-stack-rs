---
id: data-004
title: libs/db → Rust (SQLx + Postgres)
status: done
priority: critical
tags:
- data
dependencies:
- data-001
assignee: developer
created: 2026-09-22T00:33:54.665485393Z
estimate: 6h
complexity: 7
area: data
---

# libs/db → Rust (SQLx + Postgres)

## Causation Chain
> Trace the data lifecycle: schema → migration → connection pool →
query execution → result mapping → cache invalidation. Check actual
transaction boundaries and rollback behavior in code.

## Pre-flight Checks
- [ ] Read dependency task files for implementation context (Session Handoff)
- [ ] `grep -r "SELECT\|INSERT\|query\|execute" src/` - Find queries
- [ ] Check actual transaction boundaries
- [ ] Verify migration files match schema expectations
- [ ] `git log --oneline -10` - Check recent related commits

## Context
[Why this task exists and what problem it solves]

## Tasks
- [ ] [Specific actionable task]
- [ ] [Another task]
- [ ] Build + test + run to verify

## Acceptance Criteria
- [ ] [Testable criterion 1]
- [ ] [Testable criterion 2]

## Notes
[Technical details, constraints, gotchas]

---
**Session Handoff** (fill when done):
- Changed: [files/functions modified]
- Causality: [what triggers what]
- Verify: [how to test this works]
- Next: [context for dependent tasks]