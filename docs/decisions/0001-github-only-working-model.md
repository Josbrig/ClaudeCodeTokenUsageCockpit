# 0001 GitHub-only working model

- Date: 2026-10-04
- Status: accepted

## Context

The project is developed largely by an AI coding agent (Claude Code) under the direction of the project owner. Planning, requirements, code and review need one place that is traceable and that a new agent session can resume from without prior context.

## Decision

All project work happens on GitHub only:

- **Requirements** live versioned in `docs/requirements.md` and change only through pull requests. Only the owner approves them.
- **Work items** are GitHub issues created from issue forms; status is tracked with `status:*` labels; milestones group the work; epics use sub-issues.
- **Every change** goes through a branch and a pull request; `main` is protected by a ruleset with a required CI check.
- **Merges** use merge commits; feature branches are kept after merging.
- **Decisions** with impact beyond one issue are recorded as ADRs in `docs/decisions/`.

## Consequences

- The issue and pull request history is the project's memory; nothing is deleted.
- Because the repository is public, everything written here (code, commits, issues, comments) is public from the first character.
- Because the agent currently works under the owner's account, it cannot formally approve its own pull requests; the required CI check and the documented self-review replace a mandatory approval.
