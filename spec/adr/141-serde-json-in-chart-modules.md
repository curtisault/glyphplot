# ADR-141: serde_json in chart modules

Status: Accepted (2026-10-18), by the owner (open question 8 of the plan)

## Context

A chart reads options as JSON, writes its meta into the manifest, takes a
data descriptor (ADR-104) and reads and writes its view (ADR-113). unicel uses
serde and serde_json for options and meta. `discovery/budgets.md` (decision 2)
weighed:

- **(a) serde and serde_json in every module**, as unicel.
- **(b) Options through typed setters from TypeScript**, meta written natively
  into the manifest, and no JSON in the module.
- **(c) A smaller JSON crate** such as nanoserde or miniserde, not measured
  **(unverified)**.

Measured (wasm-pack release with unicel's profile, `gzip -9`):

- serde with serde_json, options parsed and metas written by derive, added
  33,088 B to a 16,209 B line module (budgets.md ladder, step 3 to step 4).
- With serde in, serde, serde_json and the float formatting and parsing they
  bring were about half of the full module (budgets.md, twiggy 0.7 by owner).
- Options through serde_json's `Value` alone took a base module from 8,269 B
  to 19,195 B (`discovery/data.md`, its `options` feature).
- A 350 KB gate (ADR-133) holds a module with serde at 49,297 B with room.

## Decision

- Every chart module links serde and serde_json for options, meta, the data
  descriptor and the view; they are among the core's only normal
  dependencies (REQ-ARCH-010).
- Options behave as unicel's (REQ-OPT-010), and errors name the bad option or
  say `options are not a JSON object: <message>` (REQ-REG-010, REQ-WASM-020).
- serde never carries data values (ADR-104) or input on the hot path
  (ADR-115).
- `mise run size --charts` shows serde's share of each module (REQ-SIZE-050).

## Consequences

- Errors name the field at fault, and the module matches unicel's shape.
- Every module repeats about 33 KB, so a page of five families pays it five
  times.
- Revisit with (b) when a module passes the 175 KB warning (REQ-SIZE-040): it
  is the one change that removes two thirds of a small module.
