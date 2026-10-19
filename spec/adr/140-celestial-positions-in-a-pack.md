# ADR-140: Celestial positions in a pack

Status: Proposed (2026-10-18)

## Context

The brief's open question 5: does the library compute celestial positions or
expect them? Stars come from catalogues as data; planets can be computed.
`discovery/data.md` and `discovery/domains.md` weighed:

- **The library computes**, with VSOP87 or with Keplerian elements.
- **The library expects positions** as data and only converts frames and
  projects. Pages must then fetch them, and JPL Horizons sends no CORS header
  (domains.md, checked).
- **Expect positions, plus an astronomy pack** that computes planets.

Measured:

- JPL's approximate Keplerian elements (Table 1) added 6,931 B gzipped to a
  wasm-bindgen module and computed eight planets in 1.5 µs natively; VSOP87A
  added 679,661 B (data.md, wasm-pack release, `gzip -9`).
- Against VSOP87A on 2026-10-18 the Keplerian model was 4.6″ to 297.1″ off,
  Saturn worst (data.md, native).
- Against Horizons over 2026, daily, geocentric: Mars within 35″, Jupiter
  within 123″ (domains.md, `kepler_vs_horizons.py`). A probe of the same with
  alt and az built to 6,460 B gzipped.
- One braille dot on a whole-sky chart 320 columns wide spans 2,025″
  (domains.md, arithmetic), so the error is under a sixteenth of a dot.
- The Keplerian tables do not cover the Moon. data.md suggested a short lunar
  series in the pack; its size was not measured **(unverified)**.

## Decision

- Chart modules compute no planetary or lunar positions; a symbol check over
  each chart module shows none of the pack's (REQ-SKY-020).
- The `sky` type takes positions in frame `icrs`, `ecliptic` or `altaz`, in
  degrees, and projects them; given an observer and a time it converts `icrs`
  to `altaz` within 0.1° (REQ-SKY-010).
- The astronomy pack computes the geocentric RA and Dec of the eight planets
  from JPL's Keplerian elements, within 3′ of Horizons fixtures for every day
  of a committed year (REQ-ASTRO-010), with alt and az, precession, phase
  folding and a Horizons parser (REQ-ASTRO-020, REQ-ASTRO-030).
- The Moon, comets, asteroids and precise tables come from Horizons as data.
- No module links VSOP87 (REQ-SIZE-060).

## Consequences

- A page can show tonight's planets with no data source.
- A live Moon needs fetched data until a lunar series is measured.
