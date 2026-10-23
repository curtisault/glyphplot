// 2 · Live candles of Wikimedia edits (REQ-SITE-040, REQ-SITE-050, ADR-136):
// a real public feed, not a market. The reader's browser opens Wikimedia
// EventStreams' `recentchange` stream (an EventSource; CORS `*`), counts the
// events of type `edit` each second, and folds the counts into one candle a
// minute: open the minute's first second, high and low its busiest and
// quietest, close its latest, volume every edit in it. Seconds are counted
// only while the stream is open, so an outage leaves a gap, not zeros. The
// stream closes connections after 15 minutes; EventSource reconnects on its
// own. When it cannot, the status line says so and the chart keeps what it
// has.

import type { Example } from "./api.ts";
import type { Candle } from "../src/data/market.ts";
import { row } from "../src/data/market.ts";
import { Scope, status } from "./support/ui.ts";

const STREAM = "https://stream.wikimedia.org/v2/stream/recentchange";
const SOURCE = "Wikimedia EventStreams (recentchange), Wikimedia Foundation; licence not stated for the event data: the chart shows counts of edits, which are facts";

export const example: Example = {
  slug: "wikimedia-edits",
  seed: null,
  start() {
    // No made-up history: the chart starts empty and fills from the feed.
    const empty = (name: string, type: "timestamp" | "f64") => ({ name, type, values: [] as number[] });
    return {
      type: "candles",
      data: { version: 1, tables: { edits: { columns: [empty("t", "timestamp"), empty("open", "f64"), empty("high", "f64"), empty("low", "f64"), empty("close", "f64"), empty("volume", "f64")] } } },
      options: { retain: { rows: 240 } },
      attrs: { label: "Wikimedia edits a second, one candle a minute, live" },
    };
  },
  live(el, host) {
    const s = new Scope();
    const out = status(s, host, `Connecting to ${SOURCE}.`);
    let es: EventSource | null = null;
    let count = 0;
    let second = Math.floor(Date.now() / 1000);
    let candle: Candle | null = null;
    let minutes = 0;

    const down = (): void => {
      out.textContent = "The Wikimedia feed is not reachable right now. The chart keeps the minutes it has, and the page tries again on its own.";
    };
    const connect = (): void => {
      es?.close();
      if (typeof EventSource !== "function") {
        down();
        return;
      }
      es = new EventSource(STREAM);
      es.onopen = () => {
        out.textContent = `Live from ${SOURCE}.`;
      };
      es.onmessage = (e: MessageEvent<string>) => {
        try {
          if ((JSON.parse(e.data) as { type?: string }).type === "edit") count++;
        } catch {
          // a message that is not JSON is not an edit
        }
      };
      es.onerror = () => {
        down();
        // A closed source does not come back by itself: try again in 30 s.
        if (es?.readyState === 2) s.after(30_000, connect);
      };
    };
    connect();
    s.add(() => es?.close());

    s.every(200, () => {
      const now = Math.floor(Date.now() / 1000);
      if (now === second) return;
      const n = count;
      count = 0;
      const open = es?.readyState === 1;
      const at = second;
      second = now;
      if (!open) return; // a second the stream was closed for is not a quiet second
      const t = Math.floor(at / 60) * 60_000;
      if (!candle || candle.t !== t) {
        candle = { t, open: n, high: n, low: n, close: n, volume: 0 };
        minutes++;
      }
      candle.high = Math.max(candle.high, n);
      candle.low = Math.min(candle.low, n);
      candle.close = n;
      candle.volume += n;
      el.append(row(candle));
      out.textContent = `Live from ${SOURCE}. ${n} edits in the last second; ${minutes} minute${minutes === 1 ? "" : "s"} drawn.`;
    });
    return s.stop;
  },
};
