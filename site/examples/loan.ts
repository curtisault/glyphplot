// 52 · A loan, paid down: what each year's payments go to, principal and
// interest stacked. The loan is 300,000 at 6 % a year over 25 years, paid
// monthly (the standard annuity payment, computed here), with an extra
// payment of principal in some years, drawn from a seeded generator, which
// shortens the term. The numbers are worked out, not observed.

import type { Example } from "./api.ts";
import { rng } from "../src/data/rng.ts";

const SEED = 52;

export function schedule(seed: number): { year: string[]; principal: number[]; interest: number[] } {
  const r = rng(seed);
  const rate = 0.06 / 12;
  const months = 25 * 12;
  let balance = 300_000;
  const payment = (balance * rate) / (1 - (1 + rate) ** -months);
  const out = { year: [] as string[], principal: [] as number[], interest: [] as number[] };
  for (let y = 1; balance > 0.005; y++) {
    let p = 0;
    let i = 0;
    for (let m = 0; m < 12 && balance > 0.005; m++) {
      const interest = balance * rate;
      const toPrincipal = Math.min(balance, payment - interest);
      balance -= toPrincipal;
      p += toPrincipal;
      i += interest;
    }
    // Some years end with an extra 5,000 to 20,000 of principal.
    if (balance > 0 && r() < 0.3) {
      const extra = Math.min(balance, 5000 + Math.floor(r() * 4) * 5000);
      balance -= extra;
      p += extra;
    }
    out.year.push(`Y${y}`);
    out.principal.push(Math.round(p));
    out.interest.push(Math.round(i));
  }
  return out;
}

export const example: Example = {
  slug: "loan",
  seed: SEED,
  start() {
    return { type: "bar", data: schedule(SEED), options: { stack: true, title: "Paid a year: principal and interest" } };
  },
};
