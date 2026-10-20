# Schemas

JSON Schemas (draft 2020-12) for every data shape that crosses a boundary in
glyphplot. Each `$id` is
`https://github.com/curtisault/glyphplot/spec/schemas/<file>`, and files refer
to each other by that `$id`. A change to a boundary shape changes its schema
in the same commit (ROADMAP §21, instruction 6).

| File | The data | Requirements |
|---|---|---|
| [data.schema.json](data.schema.json) | Chart data: named tables of typed columns and named matrices, plus the columns and rows shorthands | REQ-DATA-010, 020, 030, 050, 080, 090, 100, 120, 160 |
| [view.schema.json](view.schema.json) | The view state, whole or partial (domains, follow, cursor, selection, hidden, time, motion) | REQ-VIEW-010–060, REQ-LINK-010 |
| [input.schema.json](input.schema.json) | One input event or a list (move, down, up, leave, wheel, pinch, pan, tap, press, key) | REQ-INPUT-010–040, REQ-TOUCH-010, REQ-CONTRACT-030 |
| [events.schema.json](events.schema.json) | `{type, detail}` of each `plot-*` event the element dispatches | REQ-EVENT-010–030, REQ-EL-120, REQ-HIT-020, REQ-STREAM-070, REQ-CONTRACT-030 |
| [meta.schema.json](meta.schema.json) | A chart type's `Meta` (slug, name, note, family, modes, shapes, options, min, drop, mono, charset, budget) | REQ-CHART-020, 030, REQ-LAYOUT-070, REQ-REG-020, REQ-CHAR-010 |
| [options.schema.json](options.schema.json) | The options every chart shares (mode, encode, locale, timeZone, theme, legend, panes, scales, axes, annotations, sample, retain, drag, wheel, patterns); further keys are a chart's own | REQ-OPT-010, 020, REQ-SCALE-020, REQ-TICK-050, REQ-FMT-020, REQ-LAYOUT-050, 060, REQ-AXIS-010, REQ-ANNOT-010, REQ-SAMPLE-060, REQ-STREAM-020, 030, REQ-INPUT-040 |
| [theme.schema.json](theme.schema.json) | Theme tokens (ground, ink, muted, rule, accent, selection, up, down, series, scale, diverging) and `paper` | REQ-THEME-010, 020, 090, REQ-COLOR-020–040 |
| [locale.schema.json](locale.schema.json) | The locale pack: separators, grouping, minus, percent, numerals, currency, names, patterns, hour cycle, direction | REQ-I18N-020, 030, 060, 070, REQ-FMT-050, 060 |
| [zone.schema.json](zone.schema.json) | A zone's offset transitions over a span | REQ-TZ-010–040 |
| [manifest.schema.json](manifest.schema.json) | The data exports of `web/pkg/modules.js` (charts, modules, fallback, budgets) | REQ-MANIFEST-010, 020, REQ-LOAD-010 |
| [sizes.schema.json](sizes.schema.json) | `web/pkg/sizes.json`: module → gzipped bytes | REQ-MANIFEST-010, REQ-SIZE-010 |
| [budgets.schema.json](budgets.schema.json) | `web/pkg/budgets.json`: module → `{gzip, frameMs, worstMs, inputMs}`; `$defs/budget` is the one budget shape | REQ-MANIFEST-010, REQ-SIZE-010, REQ-PERF-030, 050, REQ-CONTRACT-050 |

## Examples

`examples/<name>.json` and `examples/<name>.<what>.json` must validate against
`<name>.schema.json`; `examples/invalid/<name>.<what>.json` must fail it. Every
schema has at least one of each.

## Checking

From the repository root:

```sh
mise exec -- uv run --no-project --with 'jsonschema[format]==4.25.1' --with referencing python -I spec/schemas/validate.py
```

It checks each schema against the 2020-12 metaschema, that its `$id` names its
file and that every property has a description; resolves `$ref`s between files
by `$id`; validates every example with formats checked; and exits 1 on any
failure.
