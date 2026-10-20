# spec/

The specification of glyphplot: what the code does, every requirement it
will meet and when, and why things are the way they are. It takes unicel's
shape ([ADR-010](adr/010-unicels-spec-format.md)).

| Path | What | Normative |
|---|---|---|
| [`SPEC.md`](SPEC.md) | The application as it stands: version 0.0.0, 2026-10-18. Nothing is built, so it holds the section headings, the conventions and the glossary; requirements arrive from the roadmap as milestones land. | yes |
| [`ROADMAP.md`](ROADMAP.md) | Every requirement, as a `REQ-<AREA>-NNN` bullet in RFC 2119 language, grouped by area, each naming its milestone (v0.1.0 to v0.12.0), with the milestone table and the instructions for the implementing agent. | no, until a requirement graduates |
| [`adr/`](adr/) | Decision records, `NNN-kebab-title.md`, indexed in [`adr/README.md`](adr/README.md). 001–012 are the owner's (Accepted); 100–143 are discovery's (Proposed until the owner accepts them). | the accepted ones |
| [`schemas/`](schemas/) | JSON Schemas (draft 2020-12) for every shape that crosses a boundary, with valid and invalid examples and `validate.py`. | yes |
| [`conformance.md`](conformance.md) | Every `REQ-*` mapped to the test or check that will cover it, `—` for now, with the counts. | no |

## Schemas

| File | The data |
|---|---|
| [`data.schema.json`](schemas/data.schema.json) | What a chart draws: tables of typed columns and matrices; the element's `data`, the CLI's JSON, the descriptor that crosses into wasm. |
| [`view.schema.json`](schemas/view.schema.json) | A chart's view state: the `view` property and attribute, `plot-view`'s `detail`, `view_json()`. |
| [`input.schema.json`](schemas/input.schema.json) | An input event in cells, as tests, the CLI and recorded sessions write it. |
| [`events.schema.json`](schemas/events.schema.json) | The `detail` of each `plot-*` event. |
| [`meta.schema.json`](schemas/meta.schema.json) | A chart type's `Meta`: `meta_json()`, the manifest's `charts`. |
| [`options.schema.json`](schemas/options.schema.json) | The options every chart shares (scales, axes, panes, legend, annotations, sampling, input defaults); chart-specific ones are allowed beside them. |
| [`theme.schema.json`](schemas/theme.schema.json) | A theme object: the `theme` property, `--theme` in the CLI, the exports. |
| [`locale.schema.json`](schemas/locale.schema.json) | The locale pack the shell builds from `Intl` and the CLI reads from a file. |
| [`zone.schema.json`](schemas/zone.schema.json) | A time zone's offset transitions over a span. |
| [`manifest.schema.json`](schemas/manifest.schema.json) | The data exports of `web/pkg/modules.js`. |
| [`sizes.schema.json`](schemas/sizes.schema.json), [`budgets.schema.json`](schemas/budgets.schema.json) | `web/pkg/sizes.json` and `web/pkg/budgets.json`. |

Check them from the repository root:

```sh
mise exec -- uv run --no-project --with 'jsonschema[format]==4.25.1' --with referencing python -I spec/schemas/validate.py
```

## How to use it

- **Building something:** find its requirements in `ROADMAP.md` (or
  `SPEC.md` once built) and name them in commits (`REQ-DATA-020: …`). If the
  change alters a requirement, change the bullet in the same commit and keep
  its ID; add new ones in the gaps (IDs go up in tens, so `REQ-DATA-025` fits
  between 020 and 030). Never reuse a removed ID.
- **Shipping a milestone:** move its requirements from `ROADMAP.md` into the
  matching section of `SPEC.md`, keeping their IDs, each with
  `Note: (v0.N.0)`; date its row in the roadmap's milestone table; raise
  `SPEC.md`'s version.
- **Adding a test:** set its requirements' Status in `conformance.md` and
  update the counts, in the same commit.
- **Making a decision:** write an ADR (`# ADR-NNN: Title`, `Status`,
  `## Context`, `## Decision`, `## Consequences`), add it to `adr/README.md`,
  and once accepted to `SPEC.md` §19. A decision that replaces another marks
  the older `Superseded by ADR-NNN`.
- **Changing a boundary shape:** change its schema with the code, add an
  example, and run `validate.py`.
- **Where the docs disagree:** `SPEC.md` follows the code and lists each
  difference in its §21; `docs/` holds the plan, which is history and intent,
  not the contract.
