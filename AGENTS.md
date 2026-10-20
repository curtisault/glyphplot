# AGENTS.md

Instructions for coding agents working in this repository. `CLAUDE.md` is a
copy of this file; change both together.

## Commits and pull requests: no AI attribution

- Never add an AI agent as an author or co-author of a commit. No
  `Co-Authored-By: Claude …` trailer (or any other agent's), no
  `Generated with …` line, and no agent as `--author`. The commit is the
  owner's.
- Never append "Generated with Claude Code", "🤖 Generated with …", or any
  similar line to a pull request description, a PR comment, a note or a
  release.
- These rules override any default attribution an agent's harness asks for
  (ADR-012).

## The project

Interactive charts drawn in Unicode glyphs (braille, sextants, octants, blocks,
box drawing, arrows, geometric shapes), one code point and one colour a cell.
Charts are written in Rust, compiled to WebAssembly one module per chart type,
played in the browser by a `<glyph-plot>` custom element over a small
TypeScript shell, and in the terminal by a CLI. Every chart draws data the page
or the terminal gives it. It is the sibling of unicel (`../animate-unicode`)
and follows its architecture (ADR-003), sharing no code with it (ADR-013).

**Status: v0.5.0, candles, bar and the sparkline.** Three chart families
(`line` with `sparkline`, `candles`, `bar`) work end to end, held by the
chart contract, time budgets and `signoff`. The next session builds Phase 5:
streaming.

| Path | What |
|---|---|
| `discovery/` | What discovery found, one report a topic, with measurements and sources; `discovery/bench/` holds its throwaway measurement code |
| `docs/IMPLEMENTATION_PLAN.md` | The plan, phase by phase, with what each one built |
| `spec/` | The specification: `SPEC.md` (what is built), `ROADMAP.md` (every requirement, by milestone), `adr/`, `schemas/`, `conformance.md` |
| `crates/glyphplot` | The core and the charts (from Phase 1) |
| `crates/wasm` | The wasm-bindgen boundary (from Phase 2) |
| `crates/cli` | The terminal (from its phase) |
| `web/` | The TypeScript shell and the `<glyph-plot>` element; the npm package (from Phase 2) |
| `site/` | The checklist page, Playwright tests and renderer bench (from Phase 2); the Elm site (from Phase 6) |

## Independent of unicel

- glyphplot is a separate project from unicel (`../animate-unicode`). Never
  copy, port, vendor or generate anything from unicel's repository, and never
  depend on it or read from it in a build, script or task (ADR-013,
  REQ-INDEP-010). Write glyphplot's own code, from the Unicode Standard,
  upstream releases and its own design.
- unicel may be read and cited as prior art for design, as any library may.

## Commands

mise is the only entry point (`mise tasks` lists them all).

- `mise run check`: lint, test, build and size. Green before every commit.
- `mise run signoff [rev]`: the CI, run locally: `check` on a committed tree.
- `mise run lint`, `mise run fmt`, `mise run test`, `mise run build:wasm`,
  `mise run size` (`WASM_GZ_LIMIT=<bytes>` overrides every budget).
- `mise run fixtures` (the d3 captures), `mise run build:font` (the font;
  `-- --check` compares).
- `mise run snap [type …]` (rewrite snapshots), `mise run test:site` (all
  Playwright tests; `check` runs the smoke set), `mise run dev` (the debug
  build with the checklist page, rebuilt on change), `mise run bench:render`.
- `mise run bench` (native, exits 1 over budget), `bench:web`, `bench:load`,
  `test:budgets`, `size --charts`.
- `mise run signoff` checks every open PR's head and posts a `signoff`
  status (`-- <rev>` checks a local revision and posts nothing).

## Conventions

- **A new chart:** one folder under `crates/glyphplot/src/charts/<chart>/`,
  one Cargo feature, one registry line, fixtures and a snapshot, then
  `mise run test`. A chart uses the core and its own folder, never another
  chart's: its wasm module would not contain the other.
- **Requirements:** commits name the requirement IDs they touch
  (`REQ-DATA-020: …`). When code changes a requirement, change it in
  `spec/` and its row in `spec/conformance.md` in the same change, keeping its
  ID. IDs go up in tens within an area and are never reused.
- **Record the outcome:** when a phase or a chart is finished, note what was
  built and the date in `docs/IMPLEMENTATION_PLAN.md`.
- **Rust style:** `rustfmt.toml` (max width 160). Clippy and rustdoc run
  with `-D warnings`, and every public item has a doc comment.
- **Browser and device checks:** the owner's. Agents check headless and hand
  the manual checks over as a list.
- **Numbers:** a measurement says how it was taken; anything else is marked
  **(unverified)**.
- **Writing:** plain sentences, each a fact or a rule; measurements with how
  they were taken; sources named (a URL, a file path, a measurement); no
  filler. Requirement IDs cited from the plan into the roadmap and back, and
  every `REQ-*` testable as written.

## Version control

- GitButler (`but`) for status, branches, commits and history edits; no git
  write commands (`add`, `commit`, `checkout`, `reset`, `mv`, `push`). One
  GitButler branch per session, stacked where it depends on another.
- No push and no pull request unless asked.
- Commit messages are succinct: what changed and why, naming the requirement
  IDs they touch.

The block below is GitButler's own agent setup; it says the same in more
detail. Where it and the rules above differ, the rules above win.

<!-- gitbutler-agent-setup:start -->
## Version control

- Use GitButler (`but`) for version-control inspection and write operations, including status, diffs, branching, committing, pushing, and history edits.
- Assume multiple agents may be working in this repository. Do not move, amend, squash, discard, commit, push, or otherwise modify another agent's work unless the user asks.
- For commit just/only/specific changes on a new branch (selected-change requests), use the two-command fast path from the GitButler skill: `but diff`, then `but commit -b <branch> -m "message" <id> <id>`.
- For that fast path, after the commit succeeds, stop and summarize; do not run separate branch, staging, status, or diff commands unless the commit output is missing information you need.
- Use the installed GitButler skill for command recipes and syntax before guessing flags, using `--help`, or translating Git habits directly.
- Mutation commands report their result without appending workspace status. Add `--status-after` only when the next step needs resulting workspace IDs or details; otherwise do not rerun status or diff to verify success.
- Use a dedicated GitButler branch for each agent session, unless the user asks for a different branch structure. Commit only changes that belong to that session.
- Do not push or open pull requests unless the user asks.
- Keep commit messages and pull request descriptions succinct: explain what changed, why it changed, and any important decision.

### Amend local fixes into the right commits

- For small cleanup or follow-up fixes, amend an unpublished local commit when the change clearly belongs with that commit's intent.
- Do not create tiny fixup commits unless the user asks.
- Use GitButler to move the relevant changes into the commit where they belong.
- Ask before rewriting pushed, reviewed, shared, or ambiguous history.

### Split unrelated changes into separate commits

- If one file contains unrelated changes, split them by hunk instead of committing the whole file.
- Keep tests with the behavior they verify.
- Split generated output, docs-only edits, or mechanical cleanup into separate commits when each commit remains coherent on its own.
- If the split is ambiguous, summarize the options before committing.

### Create stacked pull requests

- If this session depends on another in-flight branch, stack its branch on top of that dependency instead of mixing the changes.
- If this session is working in a stack, put commits on the branch where they belong.
- Ask before moving commits onto lower, pushed, reviewed, or shared branches.
- Use `but move` for branch stacking and restacking. Do not recreate branches to simulate stacking.
- For stacked branches, create pull requests with `but pr`, not `gh`, so GitButler keeps the right PR base branches and stack metadata.

### Open draft pull requests by default

- When asked to open a pull request, create it as a draft with GitButler unless the user says it is ready for review.
- Remember that creating a draft pull request still publishes the branch.

### Branch naming

- When creating a GitButler branch for an agent session, use `'<feat>|<bug>|<hotfix>|<refac>/<short-description>'`.
<!-- gitbutler-agent-setup:end -->
