// A seeded synthetic market (ADR-136, REQ-SITE-110): prices from a random
// walk, folded into candles. No exchange, no real price: every chart of it is
// labelled "simulated market" with its seed. The markets pack (v0.10.0)
// replaces it.

import { int, normal, rng, round, type Rng } from "./rng.ts";
import { DAY, MINUTE } from "./table.ts";

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
export function columns(cs: readonly Candle[]): { t: Date[]; open: number[]; high: number[]; low: number[]; close: number[]; volume: number[] } {
  return {
    t: cs.map((c) => new Date(c.t)),
    open: cs.map((c) => c.open),
    high: cs.map((c) => c.high),
    low: cs.map((c) => c.low),
    close: cs.map((c) => c.close),
    volume: cs.map((c) => c.volume),
  };
}

/** A candle as one appended row. */
export function row(c: Candle): { t: Date; open: number; high: number; low: number; close: number; volume: number } {
  return { t: new Date(c.t), open: c.open, high: c.high, low: c.low, close: c.close, volume: c.volume };
}

/**
 * A market of ticks folded into candles of `period` ms. `history` gives
 * closed candles; each `tick` moves the price and returns the candle forming
 * at that time, whose `t` repeats until its period ends, so appending it
 * replaces the last candle in place (REQ-STREAM-040, REQ-CANDLE-060).
 */
export class Market {
  readonly period: number;
  #r: Rng;
  #price: number;
  #candle: Candle | null = null;

  constructor(seed: number, period: number, price = 100) {
    this.period = period;
    this.#r = rng(seed);
    this.#price = price;
  }

  /** One step: a move of up to ±0.15 %, now and then a jump ten times that. */
  #step(): number {
    const jump = this.#r() < 0.01 ? 10 : 1;
    this.#price = Math.max(1, this.#price * (1 + (this.#r() - 0.5) * 0.003 * jump));
    return round(this.#price);
  }

  /** `n` closed candles, 20 ticks each, the last ending at `end` (epoch ms, a period boundary). */
  history(n: number, end: number): Candle[] {
    const t0 = Math.floor(end / this.period) * this.period - n * this.period;
    const out: Candle[] = [];
    for (let i = 0; i < n; i++) {
      const open = round(this.#price);
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
}

/** Daily candles from `start` (a UTC midnight), `n` of them, weekends left out when `sessions`. */
export function daily(seed: number, n: number, start: number, sessions = true, price = 100): Candle[] {
  const r = rng(seed);
  const out: Candle[] = [];
  let p = price;
  for (let d = 0; out.length < n; d++) {
    const t = start + d * DAY;
    const wd = new Date(t).getUTCDay();
    if (sessions && (wd === 0 || wd === 6)) continue;
    const open = round(p * (1 + normal(r) * 0.004));
    let high = open;
    let low = open;
    let close = open;
    for (let k = 0; k < 8; k++) {
      close = round(close * (1 + normal(r) * 0.006));
      high = Math.max(high, close);
      low = Math.min(low, close);
    }
    p = close;
    out.push({ t, open, high: round(high * (1 + r() * 0.003)), low: round(low * (1 - r() * 0.003)), close, volume: int(r, 20_000, 90_000) });
  }
  return out;
}

/** `n` closes a minute apart from `start`: a walk of 0.05 % a minute, as typed arrays (a one-value table candles resample). */
export function minuteCloses(seed: number, n: number, start: number, price = 100): { t: Float64Array; close: Float64Array } {
  const r = rng(seed);
  const t = new Float64Array(n);
  const close = new Float64Array(n);
  let p = price;
  for (let i = 0; i < n; i++) {
    t[i] = start + i * MINUTE;
    p = Math.max(1, p * (1 + normal(r) * 0.0005));
    close[i] = round(p);
  }
  return { t, close };
}
