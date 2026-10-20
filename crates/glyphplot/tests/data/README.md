# Real datasets

Datasets from public sources, checked in for tests (REQ-FIX-020). Each file is
under 100 KB, public domain, and in the full form of
[data.schema.json](../../../../spec/schemas/data.schema.json): one table,
timestamps in epoch milliseconds (UTC), sorted by time. Load them with
`tests/datasets/mod.rs`; `tests/data_readme.rs` checks this list against the
directory.

All three were retrieved on 2026-10-20 with `curl` (default headers) and
converted by a Python script that reads the download and writes the JSON
below; no value was edited by hand.

| File | Source | Licence | Retrieved | Transform |
|---|---|---|---|---|
| `wti-daily.json` | U.S. Energy Information Administration, Cushing, OK WTI Spot Price FOB, daily: <https://www.eia.gov/dnav/pet/hist/LeafHandler.ashx?n=PET&s=RWTC&f=D> | Public domain (U.S. government work); cite EIA as the source: <https://www.eia.gov/about/copyrights_reuse.php> | 2026-10-20 | The weekly HTML table read Monday to Friday a row; days with no price (holidays) left out; the 250 trading days from 2025-10-15 to 2026-10-14, the last year the table held. Columns `date` (timestamp, x) and `price` (f64, y, `USD/bbl`). |
| `treasury-par-yield-2026.json` | U.S. Department of the Treasury, Daily Treasury Par Yield Curve Rates, 2026: <https://home.treasury.gov/resource-center/data-chart-center/interest-rates/daily-treasury-rates.csv/2026/all?type=daily_treasury_yield_curve&field_tdr_date_value=2026&page&_format=csv> | Public domain (U.S. government work) | 2026-10-20 | The CSV's 201 business days from 2026-01-02 to 2026-10-20, reversed to oldest first. Column `date` (timestamp, x) and one f64 column a maturity (y, `%`): `m1`, `m1.5`, `m2`, `m3`, `m4`, `m6`, `y1`, `y2`, `y3`, `y5`, `y7`, `y10`, `y20`, `y30`; a blank cell is null. |
| `usgs-earthquakes-day.json` | U.S. Geological Survey, Earthquake Hazards Program, all events of the past day: <https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_day.csv> | Public domain (U.S. government work); credit USGS: <https://www.usgs.gov/information-policies-and-instructions/copyrights-and-credits> | 2026-10-20 | The feed's 219 events from 2026-10-19T20:34:54.730Z to 2026-10-20T20:04:46.210Z (earthquakes, and the few explosions and quarry blasts the feed lists), reversed to oldest first. Columns `time` (timestamp, x), `mag` (y), `depth` (`km`), `latitude` (lat), `longitude` (lon) and `place` (string, label); other columns left out. |
