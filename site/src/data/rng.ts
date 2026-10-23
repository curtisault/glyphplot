// Seeded randomness for the gallery's generated data (REQ-SITE-110, ADR-006):
// every generator here takes a seed, so a card's build-time still and its live
// chart draw the same numbers, and every example that uses one says
// "simulated" with its seed. mulberry32 for uniforms, Box–Muller for normals.

/** A generator of uniforms in [0, 1) from a 32-bit seed (mulberry32). */
export type Rng = () => number;

/** mulberry32 over `seed`. */
export function rng(seed: number): Rng {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** A standard normal from `r` (Box–Muller; one of the pair is dropped). */
export function normal(r: Rng): number {
  const u = Math.max(r(), 1e-12);
  return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * r());
}

/** A whole number in [lo, hi] from `r`. */
export function int(r: Rng, lo: number, hi: number): number {
  return lo + Math.floor(r() * (hi - lo + 1));
}

/** `v` rounded to `d` decimals. */
export function round(v: number, d = 2): number {
  const k = 10 ** d;
  return Math.round(v * k) / k;
}
