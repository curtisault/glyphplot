# ADR-112: Zones and locales from the shell

Status: Accepted (2026-10-18), by the owner (open question 15 of the plan)

## Context

Calendar ticks depend on a time zone, and labels on a locale. A frame must not
depend on the host (ADR-009). `discovery/scales.md` (Time zones) and
`discovery/a11y-theming.md` (Internationalisation) weighed:

- **IANA tzdata in wasm**: +150.0 KB gzipped with jiff's bundled database,
  +160.9 KB with chrono-tz (scales.md, wasm-pack release, `gzip -9`).
- **ICU4X in wasm**: +24,015 B for `icu_decimal` and +219,179 B for
  `icu_datetime` (a11y-theming.md, cargo release build without wasm-opt).
- **`Intl` called per label**: +2.0 KB, but a crossing per label per frame,
  output that depends on the browser's ICU, and nothing for the CLI.
- **The shell resolves once and passes data in.** An offset-transition table
  for a year of America/New_York took 388 `Intl` lookups and 1.2–1.7 ms
  median (0.48–0.72 ms min), ten years 4.4 ms min, at 16 B a transition
  (scales.md, Node 26, 3 runs × 5). A locale pack from `Intl` is 474–822 B
  for 11 locales and 4.23 ms cold a locale (a11y-theming.md, Node).
  Without `-u-ca-gregory`, fa-IR's month names came back from the Persian
  calendar.

`discovery/data.md` (open question 4) recommended that the element default to
the page's local zone; scales.md recommended UTC.

## Decision

- Time scales default to UTC; the core reads no clock, zone or tzdata
  (REQ-TZ-010). Timestamps are UTC instants; a zone moves only ticks and
  labels, and no frame depends on the host's zone (REQ-DATA-090).
- With no zone given, the core draws in UTC; the CLI and tests use that. The
  element passes the page's own zone by default, resolved to an IANA name and
  passed in explicitly, so the frame stays a function of its inputs
  (REQ-TZ-050; the owner's answer to open question 15).
- `timeZone` is `UTC`, a fixed offset or an IANA name. The shell turns it into
  a table of offset transitions over the data's extent and the view with
  `Intl.DateTimeFormat`, and it crosses once (REQ-TZ-020); local ticks equal
  d3's under `TZ=<zone>` (REQ-TZ-030).
- The CLI builds the same table from the system's tzdata, and its frames equal
  the element's (REQ-TZ-040).
- The shell builds a locale pack from `Intl` with the Gregorian calendar
  forced, once a locale (REQ-I18N-020); without one, formatting is ASCII and
  en-US-like (REQ-I18N-030); the CLI takes `--locale-file` (REQ-I18N-070).
- No wasm module links a tz or locale database (REQ-I18N-020, REQ-SIZE-060).

## Consequences

- Frames stay pure functions of their inputs, and modules stay small.
- A page that wants local time says so; the element does not guess.
- A pan past the table's extent asks the shell for more.
- The browser's and the system's tzdata versions may differ, so the CLI and
  the element could disagree near a recent rule change **(unverified)**.
