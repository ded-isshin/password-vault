# Agent Reports

Status: historical evidence index.

Agent reports are point-in-time work logs. They are useful for audit trails, command evidence,
runtime observations, and reviewer notes, but they are not the current source of truth when they
conflict with canonical docs.

Use current docs first:

- MVP state and blockers: [../mvp-implementation-plan.md](../mvp-implementation-plan.md)
- Architecture: [../architecture.md](../architecture.md)
- API contract: [../api-contract.md](../api-contract.md)
- Threat model: [../threat-model.md](../threat-model.md)
- Observability/SRE: [../observability-sre-metrics.md](../observability-sre-metrics.md)
- Release/runbook: [../runbooks/release-and-rollout.md](../runbooks/release-and-rollout.md)
- PostgreSQL HA/migration policy:
  [../decision-briefs/2026-06-08-postgresql-ha-migrations-stability.md](../decision-briefs/2026-06-08-postgresql-ha-migrations-stability.md)

## Reading Rules

- Treat reports as evidence of what was checked at that time, not as durable policy.
- Prefer newer reports over older reports only for the exact runtime state they verified.
- Prefer canonical docs over reports for current implementation direction.
- Do not add private host paths, private IPs, kubeconfigs, credentials, raw logs, or real user data.
- Use placeholders such as `<redacted-path>`, `<redacted-ip>`, and `<redacted-secret>`.

## Retention Policy

Keep reports that contain one of these:

- runtime rollout evidence;
- security or architecture review findings;
- incident or rollback evidence;
- validation commands that are not captured elsewhere;
- important rejected options or known blockers.

Do not add new reports for routine edits when the PR description and canonical docs are enough.
