# ADR-131: An index time scale for session gaps

Status: Proposed (2026-10-18)

## Context

A price chart must not leave empty columns for nights, weekends and holidays
(`discovery/domains.md`, Markets). Two ways were weighed:

- **(a) A calendar-aware time scale** fed by exchange calendars (hours,
  holidays, half days), which skips the time a market is closed.
- **(b) An index scale over the bars present**: x is the bar's index, labels
  come from each bar's timestamp, and a gap in time takes no space.

Facts that bear on it:

- Lightweight Charts uses (b). Its horizontal axis is a logical index
  (`src/model/time-scale.ts:551-585`, `discovery/charts.md`, code read), and
  its docs say the logical range "starts from the first data point across all
  series". The docs imply that a time gap reserves no space; they do not say
  so (domains.md).
- (b) needs only the timestamps already in the data. It also serves any
  irregular series, such as a light curve's gaps between observations.
- (a) needs calendar data for each exchange, which the library would ship or
  fetch; its size was not measured **(unverified)**.
- No measurement decides between them.

## Decision

- The core provides an index time scale on which bars sit at consecutive
  indices whatever the time between them, labelled from each bar's timestamp;
  a fixture with weekend gaps draws no empty columns (REQ-SCALE-060).
- The candlestick chart and its volume pane use it by default
  (REQ-CANDLE-040).
- Trading calendars are not in the library. They may come later in the
  markets pack, as data, only to mark bars that were expected and are
  missing.

## Consequences

- The pointer maps to a bar by index arithmetic, then to its timestamp.
- Distance along x no longer shows elapsed time: a weekend and a minute look
  alike, and a gap is visible only in the labels.
- Ticks fall on bars, so a tick may name a time that is not round.
- Linked charts share the cursor in data units, a time, which an index-scale
  chart finds by searching its timestamps.
