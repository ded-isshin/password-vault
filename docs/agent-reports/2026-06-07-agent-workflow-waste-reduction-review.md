# Agent Workflow Waste Reduction Review

Status: draft.

## Goal

Review the current `password-vault` agent workflow and identify where hallucinated progress,
throwaway tasks, duplicated analysis, and write collisions can appear. This report is public-safe
and intentionally avoids private host, network, and runtime details.

## Active Context

- Repository: `password-vault`.
- Scope: process, reviewer, and analyst guidance for the MVP workflow.
- Out of scope: product code edits, infrastructure edits, GitHub settings, deployment actions, and
  runtime secrets.

## Sources Inspected

- `AGENTS.md`
- `docs/agent-reports/`
- `docs/mvp-implementation-plan.md`
- `docs/api-contract.md`
- Current uncommitted diff in the product repository.

## Current Evidence

- `AGENTS.md` already defines the right direction: single-writer model, report-only reviewers by
  default, disjoint scopes for writer agents, separate branches or worktrees for collision-prone
  writer agents, and mandatory Claude Code review for high-risk work.
- The agent-report directory contains many reports from the same day. That is useful for audit
  history, but it also creates duplicate current-state narratives unless findings are consolidated
  into canonical docs.
- There are multiple stabilization/access/observability reports that repeat similar conclusions
  about browser access, PostgreSQL HA, migrations, and observability. Repetition increased review
  cost and made it easier for stale facts to survive.
- The initial architecture report explicitly recorded that some subagents did not return final
  output before the report was finalized. Later policy correctly says unfinished subagent output must
  not be treated as accepted.
- The current uncommitted product diff is large and crosses backend auth, tests, API contract,
  implementation plan, observability docs, and auth protocol docs. This is a high-risk collision
  shape even when there is only one human-visible writer.
- Current docs still need tighter status language. For example, the MVP plan says login finish and
  login-time TOTP verification are in progress on a feature branch, while the API contract now says
  they are implemented. The code may be implemented locally, but until validation and merge are
  complete, canonical status should say `implemented locally, not merged/deployed`.
- `docs/api-contract.md` says auth start endpoints use a 16 KiB body limit, while the auth router
  currently applies a 128 KiB body limit to the whole auth router. That is a concrete example where
  a reviewer evidence gate should catch docs/code divergence before PR readiness.
- The API contract includes a planned audit endpoint example using dotted event names, while the
  implementation writes underscore-style audit event types. This is another docs/code contract drift
  point.

## Where Waste Appears

1. Too many reports become parallel sources of truth.

   Reports are useful as session evidence. They become waste when each new report restates the same
   operational facts without updating the canonical plan, runbook, API contract, or ADR that future
   work will actually read.

2. "Done" is inferred from task momentum instead of evidence.

   A feature should not be considered done because code exists or an agent said it is ready. It is
   done only after the explicit acceptance gates pass: tests, docs alignment, public-safety scan,
   reviewer findings triaged, CI, and deployment verification when applicable.

3. Agents sometimes start before a write contract exists.

   Reviewer/advisor agents can run report-only. Writer agents need a specific write scope, branch or
   worktree, output file, max runtime, and merge path. Without that, two useful outputs can collide
   in the same architecture doc, API contract, ADR, or report.

4. External advisor output can be over-applied.

   Claude Code and other advisors are valuable, but their output must be bucketed as accepted,
   rejected, corrected, or deferred. The initial reports already show why: an advisor can produce
   strong architecture critique and still be wrong about a specific source fact.

5. Research and implementation can outrun the stabilization queue.

   The MVP plan now has a good stabilization-first queue. Work outside that queue is likely waste
   unless it directly proves or de-risks one of the gates: return login, MFA, encrypted vault CRUD,
   PostgreSQL HA, backups, controlled migrations, NetworkPolicy, alerting, and synthetic journeys.

## Collision Patterns To Prevent

- Two agents editing `docs/api-contract.md` or an ADR at the same time.
- One agent updating docs while another changes endpoint behavior without a contract diff.
- A platform agent changing Helm/GitOps assumptions while a backend agent changes runtime config.
- A reviewer writing fixes directly into the same branch while the engineer is still editing.
- Subagent reports being copied into multiple docs instead of being summarized once in the canonical
  document and linked from the task report.

## Required Workflow Change

Use a work order before every non-trivial agent run:

```text
Agent purpose:
Role:
Mode: report-only | writer
Allowed write scope:
Forbidden files:
Branch/worktree:
Max runtime:
Heartbeat/checkpoint interval:
Output file:
Acceptance gates:
Stop conditions:
Integration owner:
```

If `Mode` is `writer`, `Allowed write scope` must be narrow and disjoint. If that cannot be stated
clearly, the agent must be report-only.

## Evidence Gates

Every meaningful task should pass these gates before being called complete:

- Status gate: canonical docs use one of `planned`, `implemented locally`, `merged`, `deployed`, or
  `verified live`; avoid bare `implemented` when the branch is not merged or deployed.
- Contract gate: API contract matches handler routes, request/response shapes, status codes, error
  codes, limits, and implemented/planned endpoint status.
- Test gate: tests cover the stated security behavior, not only the happy path.
- Diff gate: large cross-domain diffs are split unless a single coherent slice requires them.
- Public-safety gate: changed public docs contain no secrets, private network details, tokens,
  kubeconfig data, or private hostnames.
- Source gate: external claims cite official docs or are marked `Needs verification`.
- Reviewer gate: Claude Code/subagent findings are triaged as accepted, rejected, corrected, or
  deferred before PR readiness.
- Deployment gate: live claims require live checks; Argo CD `Synced Healthy` is deployment-state
  evidence, not product correctness evidence.

## When To Let Agents Continue

Let an agent continue when:

- it is within the agreed max runtime;
- it is making progress on the assigned scope;
- it is not touching forbidden files or leaking sensitive data;
- it reports checkpoints for long-running work;
- its output is still likely to change a blocker, decision, or acceptance gate.

This especially applies to Claude Code for architecture, auth, crypto, Kubernetes/GitOps, CI/CD, and
security review. Interrupting early creates partial reasoning and repeated reruns.

## When To Stop Agents

Stop or quarantine an agent run when:

- it starts editing outside its assigned write scope;
- it makes claims without evidence and cannot provide a source or local verification path;
- it repeats already-accepted findings without adding a new decision or gate;
- it is blocked on missing credentials, unavailable tools, or unreachable network paths;
- it proposes dangerous host, cluster, GitHub settings, or secret actions outside the approved
  task;
- it exceeds max runtime without a checkpoint;
- it produces output that would create a second source of truth instead of updating the canonical
  artifact through the integration owner.

Stopped output should be saved as `not accepted` or `needs review`, not silently mixed into docs.

## Canonical Artifact Rules

- `docs/mvp-implementation-plan.md` is the canonical current MVP queue and status page.
- `docs/api-contract.md` is the canonical API behavior contract until an OpenAPI or typed contract
  replaces it.
- ADRs are canonical for long-lived decisions.
- Runbooks are canonical for operational procedures.
- Agent reports are evidence logs, not current-state source-of-truth documents.

After each substantial report, update only the relevant canonical artifact with accepted findings.
If no canonical artifact needs an update, the report probably should be shorter.

## Recommended Immediate Cleanup

1. Normalize status wording for the current login/TOTP branch:
   `implemented locally, not merged/deployed` until CI and PR integration are complete.
2. Fix docs/code contract drift for auth body limits and audit event names before marking the auth
   PR review-ready.
3. Add the missing negative tests already identified by review: cross-site rejection for
   `login_finish`, cross-site rejection for `totp_verify`, and five-attempt MFA exhaustion.
4. Consolidate repeated browser-access, PostgreSQL HA, migrations, and observability conclusions
   into the MVP plan, observability doc, and runbook; keep older reports as dated evidence only.
5. Keep the next implementation slices inside the stabilization queue. Defer plugin systems,
   advanced integrations, product analytics beyond protected activation, and non-essential UI polish
   until return login, encrypted vault CRUD/sync, HA database, backup/restore, and alerts are proven.

## Practical Orchestration Policy

- Run fewer agents, with sharper prompts.
- Prefer one engineer writer plus one report-only reviewer for narrow implementation slices.
- Use Claude Code as a full reviewer for high-risk slices, not as a continuously interrupted
  background process.
- For each agent result, write a short integration note: accepted, rejected, corrected, deferred.
- Before spawning another agent, check whether the blocker is actually missing evidence, stale docs,
  or an oversized diff. If yes, fix that directly instead of creating more analysis.
- Treat every new task idea as guilty until it maps to a stabilization gate or user-facing MVP
  feature.

## Definition Of "Not Waste"

Work is worth doing now when it satisfies at least one condition:

- proves a current MVP acceptance gate;
- removes a blocker before real secrets;
- reduces security, data-loss, deployment, or public-repository risk;
- improves a canonical contract that implementation depends on;
- creates a test or live check that can catch a real regression;
- updates a runbook needed for safe operation.

Everything else goes to backlog with a short rationale or is dropped.

## Validation

Tested:

- Inspected `AGENTS.md`, `docs/agent-reports/`, `docs/mvp-implementation-plan.md`, and
  `docs/api-contract.md`.
- Inspected the current product repository diff and uncommitted file list.

Not tested:

- No product code, tests, deployment, GitHub settings, or infrastructure resources were changed.
- No external research was performed for this report.

## Next Step

Use this report as the checklist before continuing the current login-finish/TOTP branch. The next
productive action is to close the known auth diff evidence gaps, not to spawn more broad analysis.
