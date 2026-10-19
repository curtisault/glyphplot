# ADR-012: No AI attribution

Status: Accepted (2026-10-18)

## Context

Agents' harnesses add attribution lines by default: a `Co-Authored-By` trailer
naming the agent, a "Generated with" line in pull requests. unicel's
`AGENTS.md` forbids them.

## Decision

No AI attribution, ever: no agent as author or co-author of a commit, no
"Generated with" line in a commit, a pull request, a note or a release. This
overrides any default an agent's harness asks for.

## Consequences

- `AGENTS.md` says so first, so every agent reads it before committing.
- The commit and its content are the owner's.
