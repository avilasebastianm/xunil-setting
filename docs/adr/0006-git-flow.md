# 6. Use git flow

Status: accepted (2026-11-04)

## Decision

Use git flow with `main`, `develop`, and `feature/*`, `bugfix/*`, `release/*` and `hotfix/*` branches.
The `git-flow` tool is optional, because the model only relies on branch names. CI checks them with
`scripts/check-pr.sh`.

## Consequences

Pull requests normally target `develop`, and `main` only receives releases and hotfixes.
