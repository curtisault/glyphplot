# ADR-010: unicel's spec format

Status: Accepted (2026-10-18)

## Context

The owner builds from the spec with agents. Commits and tests name requirement
IDs. A spec could be prose, issues, or a structured document.

## Decision

glyphplot's `spec/` takes unicel's shape: `SPEC.md` with `REQ-<AREA>-NNN`
requirements in RFC 2119 language; `ROADMAP.md` with proposed requirements by
milestone that graduate into `SPEC.md` when they ship, keeping their IDs; ADRs
in `adr/`; JSON Schemas (draft 2020-12) for every shape that crosses a
boundary in `schemas/`; and `conformance.md` mapping every requirement to its
test.

## Consequences

- IDs are stable and never reused; they go up in tens within an area.
- Since nothing is built, every requirement starts in `ROADMAP.md`, and
  `SPEC.md` starts as a 0.0.0 skeleton.
- A change to a requirement changes it and `conformance.md` in the same commit.
