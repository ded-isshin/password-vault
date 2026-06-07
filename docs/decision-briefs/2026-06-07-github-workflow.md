# Decision Brief: GitHub Workflow

Status: draft.

## Question

How should this public security-sensitive repository use GitHub for issues, branches, commits, pull
requests, project tracking, and documentation?

## Short Answer

Use GitHub Flow: short-lived topic branches from `main`, issue-linked pull requests, required CI,
review before merge, and squash merge back to `main`.

Use GitHub Projects for planning visibility. Keep durable decisions in repository files, not only in
chat or Project fields.

## GitHub Project Views

A Project view is a saved way to look at the same Project items. It is not a separate project.
GitHub supports table, board, and roadmap-style layouts.

Recommended views:

- `Backlog`: table view, grouped or filtered by status/priority.
- `Board`: kanban view by Status.
- `High Risk`: table filtered to `Risk: High`.
- `Decisions`: table filtered to ADR/research work.
- `MVP`: table filtered to MVP-phase work.
- `Roadmap`: later, once dates become useful.

The current public Project is:

- https://github.com/users/ded-isshin/projects/2

## Issue Rules

Every meaningful task should have an issue before implementation unless it is an emergency local fix.

Issue types:

- task;
- research;
- ADR;
- security review;
- bug once product code exists.

Each issue should include:

- goal;
- scope;
- non-goals;
- acceptance criteria;
- risk level;
- required artifacts;
- sources or evidence when relevant.

## Branch Rules

Branch naming:

```text
docs/<issue>-<slug>
feat/<issue>-<slug>
fix/<issue>-<slug>
sec/<issue>-<slug>
research/<issue>-<slug>
```

Examples:

```text
docs/1-threat-model-v1
research/2-auth-protocol
sec/4-totp-custody
feat/12-vault-item-create
```

## Commit Rules

Use clear, small commits. Conventional Commit-style prefixes are useful but not sacred.

Good examples:

```text
docs: add auth crypto decision brief
research: compare cloudnativepg durability modes
feat: add initial account registration endpoint
test: add totp replay rejection case
```

Avoid commits that mix unrelated changes, such as editing crypto docs and changing CI in one commit.

## Pull Request Rules

Use draft PRs early for non-trivial work. Mark PRs ready only when:

- scope is clear;
- docs are updated;
- CI passes;
- public-safety scan passes;
- risks and tests are documented;
- Claude Code or another independent reviewer is used for high-risk architecture/security changes.

PR descriptions should link the issue with `Closes #...` only when the PR fully completes the issue.

## Main Branch Ruleset Direction

Recommended `main` protection:

- require PR before merge;
- require status checks;
- require conversations resolved;
- block force pushes;
- block branch deletion;
- require linear history;
- require CODEOWNERS review for sensitive paths after CODEOWNERS is stable.

Sensitive paths:

- `.github/workflows/`;
- `docs/adr/`;
- `docs/security/`;
- future `src/auth/`;
- future `src/crypto/`;
- future deployment/chart paths.

## Actions Security

Public repository Actions must use GitHub-hosted runners only.

Default workflow posture:

- explicit minimal `permissions`;
- no secrets on untrusted pull requests;
- avoid `pull_request_target` for code execution;
- no kubeconfig in product CI;
- deploy only by GitOps PR into the infrastructure repository;
- enable GitHub security features such as secret scanning, push protection, Dependabot, and code
  scanning where available.

## Documentation Rules

Architecture and security decisions must become files:

- ADRs for decisions;
- research notes for source-backed analysis;
- runbooks for operational procedures;
- session reports for substantial orchestration work.

GitHub Projects and issue comments are useful for tracking, but they are not the final source of
truth for architecture.

## Sources

- https://docs.github.com/en/issues/planning-and-tracking-with-projects
- https://docs.github.com/en/issues/planning-and-tracking-with-projects/customizing-views-in-your-project
- https://docs.github.com/en/repositories/creating-and-managing-repositories/best-practices-for-repositories
- https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets
- https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets
- https://docs.github.com/en/actions/reference/security/secure-use
- https://git-scm.com/book/en/v2/Distributed-Git-Distributed-Workflows
- https://git-scm.com/book/en/v2/Git-Branching-Branching-Workflows
