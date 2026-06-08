# Implementation Report: CI Path Filter Optimization

Date: 2026-06-08.

Status: public-safe CI/CD optimization report.

## Goal

Reduce avoidable GitHub Actions work and prevent docs-only pushes from publishing a new runtime
container image.

## Context

A docs-only current-state report PR still ran Rust, PostgreSQL migration, Helm, PR smoke, and
container workflows. On `main`, the container workflow can publish a new GHCR image even when the
runtime code did not change. That creates image churn and can make deployment provenance noisier.

## Change

Added `paths` filters to runtime-specific workflows:

- `.github/workflows/container.yml`
- `.github/workflows/rust.yml`
- `.github/workflows/helm.yml`

The `docs` and `public-safety` workflows remain unfiltered so documentation and public repository
safety checks continue to run on normal documentation changes.

## Rationale

GitHub Actions supports `paths` filters for `push` and `pull_request` events. A workflow runs only
when at least one changed file matches its configured path patterns. Runtime workflows should not run
for pure documentation changes when the documentation workflow and public-safety workflow already
cover that change type.

This is especially important for the container workflow: runtime image publishing should be tied to
runtime-relevant changes, not report or README updates.

## Caveat

GitHub documentation notes that skipped workflows can leave required checks pending if those skipped
workflow checks are configured as required branch-protection checks. If branch protection is enabled
later, required checks should be reviewed so docs-only PRs are not blocked by intentionally skipped
runtime workflows.

## Validation

Tested:

- YAML parsed successfully for every workflow file.
- `git diff --check` passed.

Not tested:

- A docs-only PR after these filters, because the filter change itself intentionally touches the
  filtered workflow files and will trigger those workflows once.

## Sources

- GitHub Docs, Workflow syntax for GitHub Actions:
  <https://docs.github.com/en/actions/writing-workflows/workflow-syntax-for-github-actions>

