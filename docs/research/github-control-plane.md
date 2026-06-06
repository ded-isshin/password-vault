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

## Recommended GitHub Project Fields

- Status: Inbox, Backlog, Ready, In progress, In review, Blocked, Done.
- Priority: P0, P1, P2.
- Area: Crypto, Auth, Backend, Frontend, DB, Kubernetes, CI, Docs.
- Risk: Low, Medium, High.

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

## Current GitHub Project Blocker

`gh project list` works, but creating a project currently fails because the active token has
`read:project` and GitHub requires the `project` scope for project creation. Run:

```bash
gh auth refresh -s project
```

After that, create `Password Vault MVP` and add the existing startup issues.

## Sources

- https://docs.github.com/en/issues/planning-and-tracking-with-projects
- https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets
- https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners
- https://docs.github.com/en/actions/concepts/security/github_token
- https://docs.github.com/en/actions/tutorials/authenticate-with-github_token
- https://docs.github.com/en/actions/how-tos/secure-your-work/security-harden-deployments/oidc-in-hashicorp-vault
