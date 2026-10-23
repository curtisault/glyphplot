// The gallery's build-time stills (REQ-SITE-120): each example's `start()`
// drawn through the JS API with the release modules (web/dist over web/pkg,
// as the Node tests load them), its first frame written as text at the size
// of a card and of an example's page. `<gp-example>` shows the text in a
// `<pre>` until its card comes near the screen, so the gallery fetches no wasm
// before the reader scrolls.
//
//   node scripts/examples-stills.mjs     (mise run examples:stills)
//
// Writes site/examples/stills.json (cards) and site/examples/stills-full.json
// (pages), `{ "<slug>": "<rows joined by \n>" }`, both generated and
// gitignored (site/examples/.gitignore). Needs `mise run build:wasm` and
// `npm run build --prefix web`. Exits 1, naming each, when an example fails
// to draw. Fetches nothing.
import { existsSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const url = (p) => pathToFileURL(join(ROOT, p)).href;

for (const need of ["web/pkg/modules.js", "web/dist/index.js"]) {
  if (!existsSync(join(ROOT, need))) {
    console.error(`examples:stills: no ${need}: run \`mise run build:wasm\` and \`npm run build --prefix web\``);
    process.exit(1);
  }
}

const gp = await import(url("web/dist/index.js"));
const { examples } = await import(url("site/examples/index.ts"));
const { CARD, FULL, gridFor } = await import(url("site/examples/support/sizes.ts"));

/** Every zone table covers 2024 to 2028, where every example's times fall. */
const ZONE_SPAN = [Date.UTC(2024, 0, 1), Date.UTC(2028, 0, 1)];

/** The first frame of `s` (an example's Start) at `size`, as text. */
async function still(s, size) {
  const attrs = s.attrs ?? {};
  const { cols, rows } = gridFor(attrs, size);
  const c = gp.chart(s.type, {
    data: s.data,
    options: s.options ?? {},
    view: s.view,
    cols,
    rows,
    mono: "mono" in attrs,
    locale: attrs.locale ?? "en-US",
  });
  try {
    if (attrs.timezone) {
      const zone = gp.resolveZone(attrs.timezone);
      if (zone) c.zone = gp.zoneTable(zone, ZONE_SPAN[0], ZONE_SPAN[1]);
    }
    await c.ready;
    c.frame(0, attrs.theme === "paper");
    return c.text();
  } finally {
    c.free();
  }
}

const cards = {};
const pages = {};
const failed = [];
const began = performance.now();
for (const ex of examples) {
  try {
    const s = ex.start();
    cards[ex.slug] = await still(s, CARD);
    pages[ex.slug] = await still(s, FULL);
  } catch (e) {
    failed.push(`${ex.slug}: ${e instanceof Error ? e.message : String(e)}`);
  }
}
writeFileSync(join(ROOT, "site/examples/stills.json"), `${JSON.stringify(cards)}\n`);
writeFileSync(join(ROOT, "site/examples/stills-full.json"), `${JSON.stringify(pages)}\n`);
const ms = performance.now() - began;
console.log(
  `examples:stills: ${Object.keys(cards).length} of ${examples.length} examples drawn at ${CARD.cols} × ${CARD.rows} and ${FULL.cols} × ${FULL.rows} in ${(ms / 1000).toFixed(1)} s (performance.now(), Node ${process.versions.node}); site/examples/stills.json ${Buffer.byteLength(JSON.stringify(cards))} B, stills-full.json ${Buffer.byteLength(JSON.stringify(pages))} B`,
);
if (failed.length > 0) {
  console.error(`examples:stills: ${failed.length} failed:\n  ${failed.join("\n  ")}`);
  process.exit(1);
}
