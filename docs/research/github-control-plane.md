# Research Note: GitHub Control Plane For password-vault

Status: bootstrap research note.

## Why This Matters

`password-vault` is a public security-sensitive product. GitHub should make work reviewable and
auditable without exposing the home cluster or private operational details.

## Official Documentation Checked

- GitHub Issues and Projects documentation.
- GitHub repository rulesets documentation.
- GitHub CODEOWNERS documentation.
- GitHub Actions `GITHUB_TOKEN` and permissions documentation.
- GitHub OIDC with HashiCorp Vault documentation.

## Recommended Workflow

1. Create an issue for each meaningful task.
2. Assign labels for type, area, risk, and phase.
3. Track the issue in GitHub Project once `project` scope is available.
4. Create issue-linked branches such as `feat/123-slug`, `docs/123-slug`, or `sec/123-slug`.
5. Open draft PRs early for non-trivial work.
6. Require CI before merge.
7. Use human review for security, auth, crypto, CI/CD, and infrastructure-sensitive changes.
8. Use Claude Code or another independent reviewer for architecture, security, and larger diffs.
9. Merge through PR only.
10. Deploy through GitOps PR to the infrastructure repository, not direct product CI cluster access.

This is GitHub Flow with stricter safety gates for a public security-sensitive repository.
Branches are short-lived, pull requests are the review unit, and `main` is the only stable product
line.

## Recommended GitHub Project Fields

- Status: Inbox, Backlog, Ready, In progress, In review, Blocked, Done.
- Priority: P0, P1, P2.
- Area: Crypto, Auth, Backend, Frontend, DB, Kubernetes, CI, Docs.
- Risk: Low, Medium, High.

## Recommended GitHub Project Views

GitHub Project views are saved ways to look at the same project items. They do not create new work
or a second backlog. They only filter, group, sort, or change the layout of existing issues and PRs.

Recommended initial views:

- `Backlog`: table layout, all open work, sorted by priority and risk.
- `Board`: board layout, grouped by Status for day-to-day work.
- `High Risk`: table layout filtered to high-risk auth, crypto, DB, Kubernetes, and CI work.
- `Decisions`: table layout filtered to ADR/research items where a decision is still needed.
- `MVP`: table layout filtered to MVP-phase items.

`Roadmap` can wait until target dates exist. It is not useful while the project is still in research
and ADR mode.

The working public project is `Password Vault MVP`:

```text
https://github.com/users/ded-isshin/projects/2
```

## Recommended Ruleset Direction

For `main`:

- require pull request before merge;
- require status checks;
- require conversations to be resolved;
- block force pushes;
- block branch deletion;
- require linear history if it does not slow early iteration too much;
- require CODEOWNERS review for sensitive paths once ownership is stable.

Sensitive paths:

- `.github/workflows/`
- `src/auth/`
- `src/crypto/`
- `src/security/`
- `docs/security/`
- `docs/adr/`
- deployment manifests and Helm chart paths once added.

## Actions Security Direction

- Use GitHub-hosted runners only.
- Keep `permissions` explicit and minimal.
- Default to `contents: read`.
- Do not use `pull_request_target` for untrusted code execution.
- Do not expose home-cluster kubeconfig to product CI.
- Prefer `GITHUB_TOKEN` over long-lived PATs where possible.
- Consider OIDC for external services only after a dedicated security design.

## Current GitHub Project State

GitHub Project access works. The public project exists and contains the startup research and ADR
issues. A previous private project with a similar name may remain visible to the owner; the public
project is the working project for this repository.

Project views are saved layouts and filters over the same project items. Recommended working views
are documented in
[Decision Brief: GitHub Workflow](../decision-briefs/2026-06-07-github-workflow.md).

## Sources

- https://docs.github.com/en/issues/planning-and-tracking-with-projects
- https://docs.github.com/en/issues/planning-and-tracking-with-projects/learning-about-projects/about-projects
- https://docs.github.com/en/issues/planning-and-tracking-with-projects/customizing-views-in-your-project
- https://docs.github.com/en/issues/planning-and-tracking-with-projects/customizing-views-in-your-project/changing-the-layout-of-a-view
- https://docs.github.com/en/get-started/using-github/github-flow
- https://git-scm.com/book/en/v2/Git-Branching-Branching-Workflows.html
- https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets
- https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners
- https://docs.github.com/en/actions/concepts/security/github_token
- https://docs.github.com/en/actions/tutorials/authenticate-with-github_token
- https://docs.github.com/en/actions/how-tos/secure-your-work/security-harden-deployments/oidc-in-hashicorp-vault
