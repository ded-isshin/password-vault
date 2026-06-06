# Contributing

Status: bootstrap draft.

This repository is public and security-sensitive. Keep contributions small, reviewable, and
documented.

## Workflow

1. Create or link an issue.
2. Define scope, non-goals, acceptance criteria, and risk level.
3. Create a short-lived branch from `main`.
4. Update docs before or with implementation.
5. Open a draft PR early for risky work.
6. Run local validation.
7. Request review.
8. Merge only after CI and required human approval.

## Branch Names

- `research/<topic>`
- `adr/<topic>`
- `docs/<topic>`
- `feat/<issue>-<slug>`
- `fix/<issue>-<slug>`
- `security/<issue>-<slug>`
- `infra/<issue>-<slug>`

## Pull Requests

Every PR should describe:

- goal
- linked issue
- files changed
- security impact
- public safety review
- tests and validation
- documentation updated
- deployment impact
- rollback plan when relevant

## Commit Messages

Use concise, reviewable commits. Conventional-style prefixes are encouraged:

- `docs:`
- `research:`
- `adr:`
- `feat:`
- `fix:`
- `security:`
- `ci:`
