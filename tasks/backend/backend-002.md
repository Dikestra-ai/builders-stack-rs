---
id: backend-002
title: services/payment → Axum (HMAC webhooks + provider trait)
status: done
priority: high
tags:
- backend
dependencies:
- data-002
- data-001
assignee: developer
created: 2026-09-22T00:34:01.956048700Z
estimate: 5h
complexity: 6
area: backend
---

# services/payment → Axum (HMAC webhooks + provider trait)

## Causation Chain
> Trace the service orchestration: entry point → dependency injection →
business logic → side effects → return. Verify actual error propagation
paths in the codebase.

## Pre-flight Checks
- [ ] Read dependency task files for implementation context (Session Handoff)
- [ ] `grep -r "impl.*Service\|fn.*service" src/` - Find service definitions
- [ ] Check actual dependency injection patterns
- [ ] Verify error propagation through service layers
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