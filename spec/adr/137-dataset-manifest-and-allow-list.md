# ADR-137: A dataset manifest and an allow-list

Status: Proposed (2026-10-18)

## Context

The tests, the examples and the site use third-party data, and the repository
and its packages are MIT (ADR-011). `discovery/domains.md` (Datasets) found
licences of every kind among the candidates, each read on the publisher's
page:

- Public domain: EIA, Treasury, Federal Reserve, USGS and NOAA series.
- Permissive: NAB (MIT), d3-celestial's constellation lines (BSD-3-Clause),
  MIT-BIH on PhysioNet (ODC-By 1.0).
- ShareAlike: HYG v4.1, CC BY-SA 4.0. Its naked-eye subset is 8,920 stars,
  305,537 B as CSV and 118,362 B gzipped (domains.md, from the full
  33,932,548 B file).
- NonCommercial or proprietary: Gaia DR3 (CC BY-NC 3.0 IGO), Binance,
  Coinbase.
- Large: USGS's month of earthquakes was 7,032,576 B on 2026-10-18.

The options for keeping data: ad hoc files with notes; or a manifest with
hashes and a licence allow-list checked by a script. For HYG, domains.md's
open question 3 weighed committing the subset under the site's data folder
with a notice, fetching it at site build with its hash pinned, or using
d3-celestial's stars, whose upstream source is not stated.

## Decision

- Every dataset committed or served by the site has an entry in
  `datasets/manifest.json`: publisher, URL, licence name, licence URL,
  retrieval date, size and SHA-256. A check fails when a file under
  `datasets/` or the site's data folder has no entry or a different hash
  (REQ-DATASET-010).
- No committed dataset exceeds 1 MB or carries a NonCommercial, NoDerivatives
  or proprietary licence; the check reads licence names against an allow-list
  of public domain, CC0, CC BY, ODC-By, MIT and BSD (REQ-DATASET-020).
- ShareAlike data is never in a published crate or npm package; the check
  lists package contents against the manifest (REQ-DATASET-030).
- HYG is fetched at site build with its SHA-256 pinned, and not committed.

## Consequences

- Every byte of third-party data has a stated source and licence.
- The site build needs the network for HYG.
- Large sets ship as extracts under 1 MB.
- Horizons output states no licence; committing it as test fixtures awaits
  the owner's word (domains.md open question 6).
