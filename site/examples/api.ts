// The shape of an example (REQ-SITE-040, 080, 110, 120): what the gallery's
// cards, each example's page, the build-time first frames and
// `mise run examples:check` all read. One example is one module,
// site/examples/<slug>.ts, exporting `example: Example`; site/examples/index.ts
// lists them in gallery order, and site/examples/manifest.json holds their
// metadata for the Elm site's flags and the checks.

/** What an example draws first: the JS API's `chart(type, init)` arguments, pure and seeded, so Node can render its first frame at build time. */
export interface Start {
  /** The chart type (a manifest slug: line, sparkline, candles, bar, …). */
  type: string;
  /** Data in any form the element's `data` property takes. */
  data: unknown;
  options?: Record<string, unknown>;
  view?: Record<string, unknown>;
}

/** A running example's handle: what `live` returns, stopped when the card leaves or the page closes. */
export type Stop = () => void;

/** One example. Its metadata lives in manifest.json under the same slug. */
export interface Example {
  /** kebab-case, unique; the route /examples/<slug>. */
  slug: string;
  /** The first frame's data, options and view (for the build-time still and the live chart). */
  start(): Start;
  /**
   * Optional: what runs once the chart is live (feeds, appends, linked charts, controls). `el` is the
   * `<glyph-plot>` element the card or page made from `start()`; `host` is the element around it, for
   * examples that add more (a second linked chart, a control row). Returns a function that stops it.
   */
  live?(el: HTMLElement, host: HTMLElement): Stop;
  /** "simulated" examples name their seed (REQ-SITE-110); null for real data. */
  seed: number | null;
}

/** One entry of site/examples/manifest.json (REQ-SITE-080). */
export interface ManifestEntry {
  slug: string;
  title: string;
  /** One line on what it shows. */
  what: string;
  types: string[];
  modes: string[];
  domain: string;
  /** From the plan's "Features the gallery must cover". */
  features: string[];
  /** Names in datasets/manifest.json; "synthetic" for a seeded generator. */
  datasets: string[];
  milestone: string;
  /** A live third-party feed's source and licence (REQ-SITE-050); null otherwise. */
  feed: { source: string; url: string; licence: string } | null;
}
