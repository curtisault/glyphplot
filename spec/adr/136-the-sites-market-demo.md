# ADR-136: The site's market demo

Status: Accepted (2026-10-18), by the owner (open question 13 of the plan)

## Context

The owner's one-line test is a live candlestick chart of a market feed in a
`<glyph-plot>` tag (`docs/IMPLEMENTATION_PLAN.md`, Goal). The site is public, so a feed's terms decide
whether it may be shown. `discovery/domains.md` (Market data licences) read
each publisher's terms on its own page:

- **Coinbase** Market Data Terms, updated 2026-08-07: the licence is for
  personal or research purposes, and you may not "Redistribute, display, or
  disseminate the Market Data—or any data, charts, analytics … derived from
  the Market Data … to any third party" without written consent.
- **Binance** Vision Dataset Terms v1.0, 2026-08-26: CC BY-NC-SA 4.0, plus a
  ban on hosting "the Datasets or any derivative real-time or historical data
  feeds", extended to "charts, models, and derivative analytical indicators".
- **Kraken** requires prior permission for "non-personal commercial use of
  public market data" (from its API guide via a search snippet; the full terms
  page was not read).
- Public-domain US series (EIA WTI, Treasury yields, Federal Reserve exchange
  rates) are daily closes with no volume.
- **Wikimedia EventStreams** sends `access-control-allow-origin: *` (checked
  with curl), closes connections after 15 minutes, and states no licence for
  its event data; a count of edits a second uses facts, not content.

The options: an exchange feed with written permission; synthetic data; or
candles of a public feed that is not a market.

## Decision

- The site's first example is a live candlestick of a synthetic market from
  the markets pack, labelled "simulated market" (REQ-SITE-040), the label
  required of every chart of synthetic market data (REQ-MKT-040).
- The second is a live candlestick of Wikimedia edits a second, one candle a
  minute (REQ-SITE-040).
- No exchange feed runs on the site; adapter code may be shown, not run.
- An example over a live third-party feed names the source and its licence,
  and shows a message, not an error, when the feed is down (REQ-SITE-050).

## Consequences

- The owner's test runs on a real live feed with no market licence.
- No real market price appears on the site.
- The Wikimedia example must reconnect every 15 minutes.
- Revisit if Kraken or Coinbase grants written permission; such a permission
  is revocable.
