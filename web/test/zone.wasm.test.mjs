// The zone tables the shell builds (zone.ts) are what zone.schema.json
// describes and what the module's `set_zone` takes (REQ-TZ-020); and the
// line chart draws local-time ticks by them (REQ-TZ-030), with the date
// context row under a span of under two days (REQ-TICK-070).
import { test } from "node:test";
import assert from "node:assert/strict";
import { zoneTable } from "../src/zone.ts";
import { BUILD, moduleOfType, views, textOf, schema, check } from "./support/wasm.mjs";

const HOUR = 3_600_000;
const START = Date.UTC(2026, 2, 7);

test(`${BUILD}: the shell's tables validate against zone.schema.json and set_zone takes them`, async () => {
  const validate = await schema("zone");
  const m = await moduleOfType("line");
  const c = new m.glue.Chart("line", "{}");
  try {
    for (const zone of ["UTC", "+05:30", "-08:00", "America/New_York", "Australia/Lord_Howe", "Europe/London", "Pacific/Chatham"]) {
      const table = zoneTable(zone, Date.UTC(2000, 0, 1), Date.UTC(2030, 0, 1));
      check(validate, table, zone);
      c.set_zone(JSON.stringify(table));
    }
    assert.throws(() => c.set_zone(JSON.stringify({ name: "x", transitions: [], covering: [0, 1] })), /zone/);
  } finally {
    c.free();
  }
});

/** 48 hourly points from 2026-03-07 00:00 UTC, drawn at 80 × 24 under `table`. */
async function draw(table) {
  const m = await moduleOfType("line");
  const c = new m.glue.Chart("line", "{}");
  try {
    const n = 48;
    const t = new Float64Array(n);
    const y = new Float64Array(n);
    for (let i = 0; i < n; i++) {
      t[i] = START + i * HOUR;
      y[i] = Math.sin(i / 5);
    }
    const descriptor = JSON.stringify({ version: 1, tables: { data: { length: n, columns: [{ name: "t", type: "timestamp" }, { name: "y", type: "f64" }] } } });
    const ptrT = c.reserve_column(0, 0, n);
    new Float64Array(m.memory.buffer, ptrT, n).set(t);
    const ptrY = c.reserve_column(1, 0, n);
    new Float64Array(m.memory.buffer, ptrY, n).set(y);
    c.commit(descriptor);
    c.set_zone(JSON.stringify(table));
    c.resize(80, 24);
    c.frame(0, false);
    return textOf(views(c, m.memory));
  } finally {
    c.free();
  }
}

test(`${BUILD}: local-time ticks follow the zone table (REQ-TZ-030)`, async () => {
  const span = [START - 2 * 86_400_000, START + 4 * 86_400_000];
  const utc = await draw(zoneTable("UTC", ...span));
  const india = await draw(zoneTable("+05:30", ...span));
  assert.notEqual(utc, india, "a +05:30 table moves the ticks to local midnights and hours");
  // 48 hours span under two days: the context row dates each local day.
  const last = (frame) => frame.split("\n").at(-1);
  assert.match(last(utc), /Mar 7/);
  assert.match(last(utc), /Mar 8/);
});
