// 47 · Top talkers: the ten hosts sending the most traffic, as horizontal
// bars (`orient: "horizontal"`), re-ranked every second as the traffic moves.
// Each second replaces the data whole, sorted (REQ-DATA-130: data applies at
// once, with no easing between versions); the chart is kept, only its data
// changes. Simulated hosts on a private network.

import type { Example } from "./api.ts";
import { normal, rng, type Rng } from "../src/data/rng.ts";
import { Scope } from "./support/ui.ts";

const SEED = 47;
const HOSTS = Array.from({ length: 16 }, (_, i) => `10.0.4.${11 + i * 7}`);

/** Each host's rate in Mb/s, a walk around a level of its own. */
class Traffic {
  #r: Rng;
  #v: number[];
  #level: number[];
  constructor(seed: number) {
    this.#r = rng(seed);
    this.#level = HOSTS.map(() => 20 + this.#r() * 300);
    this.#v = [...this.#level];
  }
  step(): void {
    this.#v = this.#v.map((v, i) => Math.max(1, v + ((this.#level[i] as number) - v) * 0.2 + normal(this.#r) * 40));
  }
  /** The top ten, largest first. */
  top(): { host: string[]; Mbps: number[] } {
    const ranked = HOSTS.map((h, i) => [h, Math.round(this.#v[i] as number)] as const).sort((a, b) => b[1] - a[1]).slice(0, 10);
    return { host: ranked.map(([h]) => h), Mbps: ranked.map(([, v]) => v) };
  }
}

export const example: Example = {
  slug: "top-talkers",
  seed: SEED,
  start() {
    return { type: "bar", data: new Traffic(SEED).top(), options: { orient: "horizontal", title: "Mb/s by host (simulated)" } };
  },
  live(el) {
    const s = new Scope();
    const traffic = new Traffic(SEED);
    s.every(1000, () => {
      traffic.step();
      el.data = traffic.top();
    });
    return s.stop;
  },
};
