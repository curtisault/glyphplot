// 6 · A thermostat: two days of a setpoint that changes at set times, drawn
// as steps. The `step` option says where each step rises: `after` (the
// default: the value holds until the next datum), `before` (it takes the next
// datum's value at once) or `middle` (halfway). The card shows `after`; live,
// the other two are drawn under it, linked so the crosshair moves on all
// three.

import type { Example } from "./api.ts";
import { thermostat } from "../src/data/gen.ts";
import { EPOCH, table } from "../src/data/table.ts";
import { cell, grid, Scope } from "./support/ui.ts";

const SEED = 6;

function data() {
  const { t, setpoint } = thermostat(SEED, EPOCH, 2);
  return table("thermostat", [
    { name: "t", type: "timestamp", values: t },
    { name: "setpoint", unit: "°C", values: setpoint },
  ]);
}

export const example: Example = {
  slug: "thermostat",
  seed: SEED,
  start() {
    return { type: "line", data: data(), options: { mode: "step", step: "after", title: "step: after" }, attrs: { link: "thermostat" } };
  },
  live(_el, host) {
    const s = new Scope();
    const g = grid(s, host, 2);
    for (const step of ["before", "middle"]) {
      cell(s, g, `step: ${step}`, { type: "line", rows: "10", link: "thermostat" }, { data: data(), options: { mode: "step", step } });
    }
    return s.stop;
  },
};
