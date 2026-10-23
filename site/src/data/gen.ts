// Seeded generators for the gallery (REQ-SITE-110): each takes a seed and
// returns the same numbers every time, in Node at build time and in the
// browser. None of them stands for a real measurement: an example that uses
// one says "simulated" and names the seed. Shapes are chosen to look like the
// kind of data they stand in for (a CPU's daily cycle, a tide's harmonics, an
// ECG's beats), with every constant stated here.

import { int, normal, rng, round, type Rng } from "./rng.ts";
import { DAY, HOUR, MINUTE } from "./table.ts";

/** A random walk of `n` values from `start`, each step a normal of `vol` plus `drift`. */
export function walk(r: Rng, n: number, start: number, vol: number, drift = 0): Float64Array {
  const v = new Float64Array(n);
  let x = start;
  for (let i = 0; i < n; i++) {
    v[i] = x;
    x += drift + normal(r) * vol;
  }
  return v;
}

/** `values` with `runs` stretches of 1 to `maxLen` values set missing (NaN), in place. */
export function withGaps(values: Float64Array, r: Rng, runs: number, maxLen: number): Float64Array {
  for (let k = 0; k < runs; k++) {
    const at = int(r, 1, values.length - maxLen - 2);
    const len = int(r, 1, maxLen);
    for (let i = at; i < at + len; i++) values[i] = Number.NaN;
  }
  return values;
}

/** Hour of day (0–24) of epoch ms `t`, UTC. */
const hourOf = (t: number): number => (((t % DAY) + DAY) % DAY) / HOUR;

/** CPU utilisation in percent: a daily cycle (35 ± 20 %), noise, and now and then a spike, held in 0–100. */
export function cpu(seed: number, t: Float64Array): Float64Array {
  const r = rng(seed);
  const v = new Float64Array(t.length);
  let spike = 0;
  for (let i = 0; i < t.length; i++) {
    const h = hourOf(t[i] as number);
    if (r() < 0.01) spike = 30 + r() * 40;
    spike *= 0.85;
    v[i] = round(Math.min(100, Math.max(0, 35 + 20 * Math.sin(((h - 9) / 24) * 2 * Math.PI) + normal(r) * 4 + spike)), 1);
  }
  return v;
}

/** Requests a second: a daily cycle around 1,200 with noise. */
export function requests(seed: number, t: Float64Array): Float64Array {
  const r = rng(seed);
  const v = new Float64Array(t.length);
  for (let i = 0; i < t.length; i++) {
    const h = hourOf(t[i] as number);
    v[i] = Math.round(1200 + 500 * Math.sin(((h - 10) / 24) * 2 * Math.PI) + normal(r) * 60);
  }
  return v;
}

/** Latency percentiles in ms: p50 near 40, p90 about 2.2 times it, p99 about 5 times with bursts. */
export function latency(seed: number, n: number): { p50: Float64Array; p90: Float64Array; p99: Float64Array } {
  const r = rng(seed);
  const p50 = new Float64Array(n);
  const p90 = new Float64Array(n);
  const p99 = new Float64Array(n);
  let burst = 0;
  for (let i = 0; i < n; i++) {
    if (r() < 0.02) burst = 300 + r() * 900;
    burst *= 0.8;
    const base = 40 * Math.exp(normal(r) * 0.08);
    p50[i] = round(base, 1);
    p90[i] = round(base * (2.2 + r() * 0.3), 1);
    p99[i] = round(base * (5 + r()) + burst, 1);
  }
  return { p50, p90, p99 };
}

/** Net flow: signed values over several orders of magnitude (a mostly small flow and rare large moves either way). */
export function netFlow(seed: number, n: number): Float64Array {
  const r = rng(seed);
  const v = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    const big = r() < 0.05 ? 10 ** (2 + r() * 2) : 1;
    v[i] = Math.round(normal(r) * 20 * big);
  }
  return v;
}

/** Tide in metres above a datum, every `step` ms from `start`: four harmonic constituents as a prediction, plus a slow surge for the observed level. */
export function tide(seed: number, t: Float64Array): { observed: Float64Array; predicted: Float64Array } {
  const r = rng(seed);
  // Period (hours), amplitude (m), phase (rad): M2, S2, K1, O1 at made-up amplitudes.
  const parts: [number, number, number][] = [
    [12.42, 0.68, 0.3],
    [12.0, 0.14, 1.1],
    [23.93, 0.1, 2.0],
    [25.82, 0.07, 0.6],
  ];
  const predicted = new Float64Array(t.length);
  const observed = new Float64Array(t.length);
  let surge = 0;
  for (let i = 0; i < t.length; i++) {
    const h = ((t[i] as number) - (t[0] as number)) / HOUR;
    let p = 0.8;
    for (const [period, amp, phase] of parts) p += amp * Math.cos((2 * Math.PI * h) / period + phase);
    surge = surge * 0.98 + normal(r) * 0.012;
    predicted[i] = round(p, 3);
    observed[i] = round(p + surge + normal(r) * 0.01, 3);
  }
  return { predicted, observed };
}

/** An ECG-like trace in mV at `hz` samples a second for `seconds`: P, QRS and T waves as Gaussians, about 72 beats a minute with beat-to-beat variation. */
export function ecg(seed: number, seconds: number, hz: number): Float64Array {
  const r = rng(seed);
  const n = Math.round(seconds * hz);
  const v = new Float64Array(n);
  // (offset in s from the R peak, width in s, height in mV)
  const waves: [number, number, number][] = [
    [-0.2, 0.025, 0.12],
    [-0.03, 0.01, -0.1],
    [0, 0.012, 1.1],
    [0.03, 0.01, -0.25],
    [0.25, 0.04, 0.3],
  ];
  const beats: number[] = [];
  for (let s = 0.4; s < seconds + 1; s += 60 / (72 + normal(r) * 3)) beats.push(s);
  for (let i = 0; i < n; i++) {
    const s = i / hz;
    let y = normal(r) * 0.01 + 0.05 * Math.sin(2 * Math.PI * 0.25 * s);
    for (const b of beats) {
      if (Math.abs(s - b) > 0.5) continue;
      for (const [off, w, h] of waves) y += h * Math.exp(-(((s - b - off) / w) ** 2) / 2);
    }
    v[i] = round(y, 3);
  }
  return v;
}

/** Hourly temperature (°C) and relative humidity (%), daily cycles opposed, with weather noise. */
export function weather(seed: number, t: Float64Array): { temperature: Float64Array; humidity: Float64Array } {
  const r = rng(seed);
  const temperature = new Float64Array(t.length);
  const humidity = new Float64Array(t.length);
  let front = 0;
  for (let i = 0; i < t.length; i++) {
    const h = hourOf(t[i] as number);
    front = front * 0.97 + normal(r) * 0.4;
    const daily = Math.sin(((h - 9) / 24) * 2 * Math.PI);
    temperature[i] = round(14 + 6 * daily + front, 1);
    humidity[i] = round(Math.min(100, Math.max(15, 65 - 18 * daily - front * 2 + normal(r) * 2)), 0);
  }
  return { temperature, humidity };
}

/** A thermostat's setpoint (°C) every 30 minutes over `days`: 17 at night, 21 from 06:30, 18 from 09:00, 21 from 17:30, with a seeded nudge some days. */
export function thermostat(seed: number, start: number, days: number): { t: Float64Array; setpoint: Float64Array } {
  const r = rng(seed);
  const n = days * 48;
  const t = new Float64Array(n);
  const setpoint = new Float64Array(n);
  let nudge = 0;
  for (let i = 0; i < n; i++) {
    t[i] = start + i * 30 * MINUTE;
    const h = i % 48 / 2;
    if (i % 48 === 0) nudge = r() < 0.3 ? 1 : 0;
    setpoint[i] = h < 6.5 ? 17 : h < 9 ? 21 + nudge : h < 17.5 ? 18 : h < 22.5 ? 21 + nudge : 17;
  }
  return { t, setpoint };
}

/** A currency rate near `level`, one value a business day: a walk of 0.4 % daily steps, rounded to four decimals. */
export function fx(seed: number, n: number, level: number): Float64Array {
  const r = rng(seed);
  const v = new Float64Array(n);
  let x = level;
  for (let i = 0; i < n; i++) {
    v[i] = round(x, 4);
    x *= 1 + normal(r) * 0.004;
  }
  return v;
}

/** The planetary K-index every 3 hours: quiet (0–3) with a few storms that rise to 5–8 and decay. */
export function kIndex(seed: number, n: number): Float64Array {
  const r = rng(seed);
  const v = new Float64Array(n);
  let storm = 0;
  for (let i = 0; i < n; i++) {
    if (r() < 0.03) storm = 3 + r() * 4;
    storm *= 0.8;
    const k = Math.min(9, Math.max(0, 1.5 + normal(r) * 0.8 + storm));
    v[i] = Math.round(k * 3) / 3;
  }
  return v;
}
