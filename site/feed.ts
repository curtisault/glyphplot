// A seeded synthetic market feed for the checklist (the plan's Phase 5, task
// 6): prices from a random walk on mulberry32, so every pass draws the same
// history and the same ticks. Every page showing it labels it "simulated":
// no exchange, no real prices. The markets pack replaces it later.

/** A deterministic generator (mulberry32). */
export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** One candle; `t` is its period's start, epoch ms. */
export interface Candle {
  t: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

/** Candles as the columns shorthand, `t` as Dates (a timestamp column). */
export function candleColumns(cs: readonly Candle[]): { t: Date[]; open: number[]; high: number[]; low: number[]; close: number[]; volume: number[] } {
  return {
    t: cs.map((c) => new Date(c.t)),
    open: cs.map((c) => c.open),
    high: cs.map((c) => c.high),
    low: cs.map((c) => c.low),
    close: cs.map((c) => c.close),
    volume: cs.map((c) => c.volume),
  };
}

const cents = (v: number): number => Math.round(v * 100) / 100;

/**
 * A feed of ticks folded into candles of `period` ms. {@link Feed.history}
 * gives the closed candles before `start`; each {@link Feed.tick} moves the
 * price and returns the candle forming at that time, whose `t` repeats until
 * the period ends: appended, it replaces the last candle in place
 * (REQ-STREAM-040, REQ-CANDLE-060).
 */
export class Feed {
  readonly period: number;
  #r: () => number;
  #price: number;
  #candle: Candle | null = null;

  constructor(seed = 2026, period = 5000, price = 100) {
    this.period = period;
    this.#r = rng(seed);
    this.#price = price;
  }

  /** One step of the walk: a move of up to ±0.15 %, now and then a jump of up to ±1.5 %. */
  #step(): number {
    const jump = this.#r() < 0.01 ? 10 : 1;
    this.#price = Math.max(1, this.#price * (1 + (this.#r() - 0.5) * 0.003 * jump));
    return cents(this.#price);
  }

  /** `n` closed candles ending at the period before `start` (epoch ms), 20 ticks each. */
  history(n: number, start: number): Candle[] {
    const t0 = Math.floor(start / this.period) * this.period - n * this.period;
    const out: Candle[] = [];
    for (let i = 0; i < n; i++) {
      const open = cents(this.#price);
      let high = open;
      let low = open;
      let close = open;
      for (let k = 0; k < 20; k++) {
        close = this.#step();
        high = Math.max(high, close);
        low = Math.min(low, close);
      }
      out.push({ t: t0 + i * this.period, open, high, low, close, volume: Math.round(1000 + this.#r() * 9000) });
    }
    return out;
  }

  /** A tick at `now` (epoch ms): the forming candle after it, a copy. */
  tick(now: number): Candle {
    const t = Math.floor(now / this.period) * this.period;
    const price = this.#step();
    const c = this.#candle;
    if (!c || c.t !== t) this.#candle = { t, open: c ? c.close : price, high: price, low: price, close: price, volume: 0 };
    const cur = this.#candle as Candle;
    cur.high = Math.max(cur.high, price);
    cur.low = Math.min(cur.low, price);
    cur.close = price;
    cur.volume += Math.round(10 + this.#r() * 190);
    return { ...cur };
  }

  /** A sample of the walk alone, for a line: the price after one step. */
  sample(): number {
    return this.#step();
  }
}
