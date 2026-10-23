// `<gp-example slug="…">`: one gallery card's chart, or with `full` the chart
// of an example's own page (REQ-SITE-040, REQ-SITE-120).
//
// Until it comes near the screen it shows the example's build-time still,
// the first frame as text in a `<pre>` (scripts/examples-stills.mjs writes
// site/examples/stills.json), and loads nothing else: the shell is imported
// and a `<glyph-plot>` made only then. The still moves into the
// `<glyph-plot>` as its light-DOM content, which the element keeps until its
// own first frame (REQ-EL-130). Then the example's `live(el, host)` runs, and
// its stop runs when the card leaves the document.
//
// On the gallery (no `full`) a card goes live only once the reader has
// scrolled, touched, clicked or pressed a key on the page, or pointed at or
// focused that card: the gallery fetches no wasm before the reader acts, and
// the shell loads each family's module once however many cards use it
// (REQ-LOAD-010). An example's page (`full`) goes live as soon as its chart
// is near the screen.
//
// Under the chart a line says where the data comes from: "simulated" and the
// seed for generated data (REQ-SITE-110), the publisher and licence for a
// dataset, the source and licence for a live feed (REQ-SITE-050).

/// <reference types="vite/client" />
import type { Example, ManifestEntry, Stop } from "./api.ts";
import { exampleOf } from "./index.ts";
import manifestJson from "./manifest.json" with { type: "json" };
// The repository's dataset manifest (REQ-DATASET-010): what a card says of each dataset an example names.
import datasetsJson from "../../datasets/manifest.json" with { type: "json" };
import { CARD, FULL, gridFor } from "./support/sizes.ts";

type Stills = Record<string, string>;

// The cards' stills ship with this module; the pages' load when a page asks.
// Both are generated (mise run examples:stills); without them a card shows
// an empty box of the right height.
const CARD_STILLS: Stills = Object.values(import.meta.glob<Stills>("./stills.json", { eager: true, import: "default" }))[0] ?? {};
const fullStills = Object.values(import.meta.glob<Stills>("./stills-full.json", { import: "default" }))[0];

const MANIFEST = manifestJson as ManifestEntry[];

/** A datasets/manifest.json entry, the fields a card reads. */
interface Dataset {
  id: string;
  title: string;
  publisher: string;
  licence: string;
  /** `committed` and `build`: a file the site serves or bundles; `live`: the reader's browser fetches it; `generated`: a seeded generator. */
  storage: "committed" | "build" | "live" | "generated";
}

const DATASETS = new Map((datasetsJson.datasets as Dataset[]).map((d) => [d.id, d]));

// Each example's module as text, for its page's "source" (REQ-SITE-040), loaded only when asked for.
const SOURCES = import.meta.glob<string>(["./*.ts", "!./api.ts", "!./index.ts", "!./element.ts"], { query: "?raw", import: "default" });

/** The source of example `slug`'s module (site/examples/<slug>.ts), or null when there is none. */
export async function exampleSource(slug: string): Promise<string | null> {
  const load = SOURCES[`./${slug}.ts`];
  return load ? load() : null;
}

/** The tag. */
export const TAG = "gp-example";

// ------------------------------------------------------------ the reader's first act

let woken = false;
const sleepers = new Set<GpExample>();
const ACTS = ["scroll", "wheel", "touchstart", "pointerdown", "keydown"] as const;

function wake(): void {
  if (woken) return;
  woken = true;
  for (const t of ACTS) removeEventListener(t, wake, true);
  for (const s of [...sleepers]) s.check();
  sleepers.clear();
}

function listenForWake(): void {
  if (woken) return;
  for (const t of ACTS) addEventListener(t, wake, { capture: true, passive: true });
}

// ------------------------------------------------------------ styles

const CSS = `
:where(gp-example){display:block}
:where(gp-example .gp-ex-box){position:relative;overflow:hidden;height:calc(var(--gp-ex-rows,16) * 1.2em)}
:where(gp-example .gp-ex-still){margin:0;padding:0;font:inherit;font-family:"glyphplot mono",ui-monospace,"SF Mono",Menlo,Consolas,monospace;line-height:1.2em;white-space:pre;overflow:hidden;font-variant-ligatures:none;letter-spacing:0;opacity:.85}
:where(gp-example glyph-plot .gp-ex-still){position:absolute;inset:0}
:where(gp-example .gp-ex-provenance){margin:.4em 0 0;font-size:.8em;opacity:.8}
:where(gp-example .gp-ex-controls){display:flex;flex-wrap:wrap;gap:.5em;margin:.5em 0}
:where(gp-example .gp-ex-controls button, gp-example .gp-ex-controls select){font:inherit;font-size:.9em;min-height:32px;padding:.2em .7em;border-radius:6px;border:1px solid color-mix(in srgb,currentColor 35%,transparent);background:transparent;color:inherit;cursor:pointer}
:where(gp-example .gp-ex-controls button[aria-pressed="true"]){background:color-mix(in srgb,currentColor 14%,transparent)}
:where(gp-example .gp-ex-status, gp-example .gp-ex-note){display:block;margin:.4em 0;font-size:.85em;opacity:.85}
:where(gp-example .gp-ex-grid){display:grid;grid-template-columns:repeat(var(--gp-ex-columns,2),minmax(0,1fr));gap:.75em;margin-top:.75em}
:where(gp-example .gp-ex-grid.gp-ex-dense){gap:.1em .75em}
@media (max-width:640px){:where(gp-example .gp-ex-grid){grid-template-columns:minmax(0,1fr)}}
:where(gp-example .gp-ex-caption){margin:.6em 0 .2em;font-size:.85em;opacity:.85}
:where(gp-example .gp-ex-narrow){width:70%}
:where(gp-example .gp-ex-log){margin:.5em 0;padding-left:1.5em;max-height:14em;overflow:auto;font:.8em/1.4 ui-monospace,monospace;overflow-wrap:anywhere}
:where(gp-example .gp-ex-code){margin:.5em 0;font:.8em/1.4 ui-monospace,monospace;white-space:pre-wrap;overflow-wrap:anywhere}
:where(gp-example .gp-ex-sr){margin:.5em 0;font-size:.85em}
:where(gp-example .gp-ex-table){max-height:12em;overflow:auto}
:where(gp-example .gp-ex-table table){border-collapse:collapse;font:.8em/1.3 ui-monospace,monospace}
:where(gp-example .gp-ex-table th, gp-example .gp-ex-table td){padding:0 .5em;text-align:right}
:where(gp-example .gp-ex-sentence){line-height:2.4}
`;

function injectStyle(doc: Document): void {
  if (doc.getElementById("gp-example-style")) return;
  const style = doc.createElement("style");
  style.id = "gp-example-style";
  style.textContent = CSS;
  (doc.head ?? doc.documentElement).append(style);
}

// ------------------------------------------------------------ provenance

/** What a card says of its data (REQ-SITE-050, REQ-SITE-110). */
export function provenance(ex: Example, entry: ManifestEntry | undefined): string {
  const parts: string[] = [];
  const named = entry?.datasets ?? [];
  if (ex.seed !== null) parts.push(`${entry?.domain === "markets" ? "Simulated market" : "Simulated data"}, seed ${ex.seed}.`);
  for (const name of named) {
    const d = DATASETS.get(name);
    if (!d || (d.storage !== "committed" && d.storage !== "build")) continue;
    parts.push(`Data: ${d.publisher}, ${d.title}. ${d.licence}.`);
  }
  // A feed beside generated or checked-in data is fetched when the reader asks; a feed alone is the chart.
  const onRequest = named.some((n) => DATASETS.get(n)?.storage !== "live");
  if (entry?.feed) parts.push(`${onRequest ? "On request, live" : "Live"} from ${entry.feed.source} (${entry.feed.url}). ${entry.feed.licence}.`);
  return parts.join(" ");
}

// ------------------------------------------------------------ the element

const Base: typeof HTMLElement = typeof HTMLElement === "undefined" ? (class {} as unknown as typeof HTMLElement) : HTMLElement;

/** `<gp-example>`. */
export class GpExample extends Base {
  static get observedAttributes(): string[] {
    return ["slug", "full"];
  }

  #io: IntersectionObserver | null = null;
  #near = false;
  #touched = false;
  #live = false;
  #gen = 0;
  #stop: Stop | null = null;
  #still: HTMLPreElement | null = null;
  #box: HTMLElement | null = null;
  #host: HTMLElement | null = null;

  /** The example's slug. */
  get slug(): string {
    return this.getAttribute("slug") ?? "";
  }

  /** Whether this is an example's own page. */
  get full(): boolean {
    return this.hasAttribute("full");
  }

  /** Pointing at or focusing a card wakes it alone. */
  #touch = (): void => {
    this.#touched = true;
    this.check();
  };

  connectedCallback(): void {
    injectStyle(this.ownerDocument);
    this.addEventListener("pointerenter", this.#touch);
    this.addEventListener("focusin", this.#touch);
    this.#render();
  }

  disconnectedCallback(): void {
    this.removeEventListener("pointerenter", this.#touch);
    this.removeEventListener("focusin", this.#touch);
    this.#teardown();
  }

  attributeChangedCallback(_name: string, old: string | null, value: string | null): void {
    if (old === value || !this.isConnected) return;
    this.#teardown();
    this.#render();
  }

  /** Goes live when near the screen and the reader has acted (or this is a page, or this card was pointed at). */
  check(): void {
    if (this.#live || !this.#near || !this.isConnected) return;
    if (this.full || woken || this.#touched) void this.#goLive();
    else sleepers.add(this);
  }

  #render(): void {
    const gen = ++this.#gen;
    const ex = exampleOf(this.slug);
    this.replaceChildren();
    if (!ex) {
      this.textContent = `There is no example named "${this.slug}".`;
      return;
    }
    const entry = MANIFEST.find((m) => m.slug === ex.slug);
    // The box takes the still's rows (the stills are drawn at the chart's rows), so nothing is generated yet.
    const text = CARD_STILLS[ex.slug];
    const rows = text ? text.split("\n").length : (this.full ? FULL : CARD).rows;
    const host = document.createElement("div");
    host.className = "gp-ex-live";
    const box = document.createElement("div");
    box.className = "gp-ex-box";
    box.style.setProperty("--gp-ex-rows", String(rows));
    const still = document.createElement("pre");
    still.className = "gp-ex-still";
    still.setAttribute("role", "img");
    still.setAttribute("aria-label", `${entry?.title ?? ex.slug}: a still of the chart's first frame; the live chart replaces it when it comes into view.`);
    still.textContent = text ?? "";
    box.append(still);
    host.append(box);
    const note = document.createElement("p");
    note.className = "gp-ex-provenance";
    note.textContent = provenance(ex, entry);
    this.append(host);
    if (note.textContent) this.append(note);
    this.#host = host;
    this.#box = box;
    this.#still = still;
    if (this.full && fullStills) {
      void fullStills().then((all) => {
        const page = all[ex.slug];
        if (gen !== this.#gen || this.#live || page === undefined) return;
        still.textContent = page;
        box.style.setProperty("--gp-ex-rows", String(page.split("\n").length));
      });
    }
    listenForWake();
    if (typeof IntersectionObserver !== "function") {
      this.#near = true;
      this.check();
      return;
    }
    this.#io = new IntersectionObserver(
      (entries) => {
        this.#near = entries.some((e) => e.isIntersecting);
        if (this.#near) this.check();
        else sleepers.delete(this);
      },
      { rootMargin: "100% 0px" },
    );
    this.#io.observe(this);
  }

  async #goLive(): Promise<void> {
    this.#live = true;
    sleepers.delete(this);
    this.#io?.disconnect();
    this.#io = null;
    const gen = this.#gen;
    const ex = exampleOf(this.slug);
    if (!ex) return;
    await import("glyphplot/element"); // defines <glyph-plot>; no wasm until a chart is made
    if (gen !== this.#gen || !this.isConnected || !this.#box || !this.#host) return;
    const s = ex.start();
    const attrs = s.attrs ?? {};
    const grid = gridFor(attrs, this.full ? FULL : CARD);
    const entry = MANIFEST.find((m) => m.slug === ex.slug);
    const el = document.createElement("glyph-plot") as Parameters<NonNullable<Example["live"]>>[0];
    el.setAttribute("type", s.type);
    for (const [k, v] of Object.entries(attrs)) if (k !== "rows" && k !== "cols") el.setAttribute(k, v);
    if (attrs["cols"] !== undefined) el.setAttribute("cols", attrs["cols"]);
    el.setAttribute("rows", String(grid.rows));
    if (!el.hasAttribute("label")) el.setAttribute("label", entry?.title ?? ex.slug);
    if (s.options) el.options = s.options;
    el.data = s.data as typeof el.data;
    if (s.view) el.view = s.view;
    // The still stays as the chart's light-DOM content until its first frame (REQ-EL-130).
    if (this.#still) {
      this.#still.removeAttribute("role");
      this.#still.removeAttribute("aria-label");
      this.#still.setAttribute("aria-hidden", "true");
      el.append(this.#still);
    }
    this.#box.replaceWith(el);
    this.#box = null;
    try {
      this.#stop = ex.live?.(el, this.#host) ?? null;
    } catch (e) {
      console.error(`gp-example ${ex.slug}: live: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  #teardown(): void {
    this.#gen++;
    sleepers.delete(this);
    this.#io?.disconnect();
    this.#io = null;
    const stop = this.#stop;
    this.#stop = null;
    try {
      stop?.();
    } catch (e) {
      console.warn(`gp-example ${this.slug}: stop: ${e instanceof Error ? e.message : String(e)}`);
    }
    this.replaceChildren(); // the <glyph-plot> frees its chart when it leaves (REQ-EL-140)
    this.#live = false;
    this.#near = false;
    this.#touched = false;
    this.#box = null;
    this.#host = null;
    this.#still = null;
  }
}

/** Defines `<gp-example>` once; does nothing without a DOM. */
export function defineExample(): void {
  if (typeof customElements === "undefined" || customElements.get(TAG)) return;
  customElements.define(TAG, GpExample);
}

defineExample();
