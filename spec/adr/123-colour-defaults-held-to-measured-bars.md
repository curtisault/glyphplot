# ADR-123: Colour defaults held to measured bars

Status: Proposed (2026-10-18)

## Context

Default colours must contrast with the ground (WCAG 1.4.11, 3:1 for marks)
and stay apart under colour-vision deficiency. `discovery/a11y-theming.md`
measured palettes with colour-science 0.4.6: contrast by WCAG 2.2, CVD
simulated by Machado 2009 at severity 1.0 on linear sRGB, differences in
CIEDE2000; deterministic. Smallest pairwise ΔE00, and colours under 3:1 on
the light ground `#f5f6f7`:

| Palette | Normal | Deutan | Protan | Tritan | Under 3:1, light |
|---|---|---|---|---|---|
| Okabe–Ito | 21.7 | 11.6 | 12.2 | 10.9 | 4 of 8 |
| Tableau 10 | 18.1 | 0.7 | 1.0 | 6.8 | 6 of 10 |
| Observable 10 | 18.0 | 2.6 | 6.0 | 8.2 | 7 of 10 |
| Searched, held to Okabe–Ito hues, light | 16.4 | 15.7 | 15.6 | 15.7 | 0 |

- Sequential, 9 steps: cividis keeps neighbours ≥ 7.0 under all four visions;
  viridis falls to 4.4 under protanopia.
- Diverging: RdBu's ends are 1.39:1 and 1.40:1 on the dark ground `#131518`.
- Up and down: unicel's green and red are 9.3 apart under deuteranopia; blue
  and orange at least 50.5 under every vision. Tritan figures are the least
  reliable, Machado's model being weakest there.

The options: Okabe–Ito as published; Tableau 10 or Observable 10; or palettes
searched per ground to meet the bars, chosen by the owner's eye.

## Decision

- Default series, `up`, `down` and `accent` colours are at least 3:1 against
  their default ground, `ink` and `muted` at least 4.5:1 (REQ-THEME-040).
- Each default categorical palette holds eight colours at least 15 ΔE00 apart
  under normal vision and the three simulated deficiencies (REQ-THEME-050);
  the searched candidates are in the bench, and the owner picks by eye.
- The default sequential scale is cividis; the diverging scale runs blue to
  orange with its middle on the ground's side (REQ-THEME-080).
- `up` and `down` are tokens, and candles and bars also differ by fill
  (REQ-THEME-060).
- One ink carries series by dash pattern (REQ-MONO-010), markers for series
  5–8 (REQ-MONO-020), fills `█ ▓ ▒ ░` (REQ-MONO-030) and legend keys
  (REQ-MONO-040).

## Consequences

- The defaults are no well-known palette; the docs say why.
- Past eight series, patterns carry identity (ten colours cap near 17 ΔE00).
- Dash periods under 4 dots may read as noise **(unverified)**.
