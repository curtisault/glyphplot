// zone.ts: the zone a `timeZone` names, the page's by default (REQ-TZ-050),
// and the table of offset transitions over a span, from Intl
// (REQ-TZ-020, zone.schema.json), checked against known transitions: New
// York's 2026 daylight-time dates and Lord Howe's half-hour change.
import { test } from "node:test";
import assert from "node:assert/strict";
import { zoneTable, resolveZone, fixedOffset, offsetAt, pageZone, covers, coveringFor } from "../src/zone.ts";

const DAY = 86_400_000;
const Y2026 = [Date.UTC(2026, 0, 1), Date.UTC(2027, 0, 1)];

test("New York 2026: EST, EDT from 8 March 07:00 UTC, EST from 1 November 06:00 UTC", () => {
  const t = zoneTable("America/New_York", ...Y2026);
  assert.deepEqual(t, {
    name: "America/New_York",
    transitions: [
      { at: Y2026[0], offset: -300 },
      { at: Date.UTC(2026, 2, 8, 7), offset: -240 },
      { at: Date.UTC(2026, 10, 1, 6), offset: -300 },
    ],
    covering: Y2026,
  });
  // The second before each change still has the old offset.
  assert.equal(offsetAt("America/New_York", Date.UTC(2026, 2, 8, 7) - 1000), -300);
  assert.equal(offsetAt("America/New_York", Date.UTC(2026, 10, 1, 6) - 1000), -240);
});

test("Lord Howe 2026: a half-hour change, +11:00 to +10:30 on 5 April and back on 4 October (local)", () => {
  const t = zoneTable("Australia/Lord_Howe", ...Y2026);
  assert.deepEqual(t.transitions, [
    { at: Y2026[0], offset: 660 },
    { at: Date.UTC(2026, 3, 4, 15), offset: 630 },
    { at: Date.UTC(2026, 9, 3, 15, 30), offset: 660 },
  ]);
});

test("a zone with one offset over the span has one transition; UTC and fixed offsets have one", () => {
  assert.deepEqual(zoneTable("Asia/Kolkata", ...Y2026).transitions, [{ at: Y2026[0], offset: 330 }]);
  assert.deepEqual(zoneTable("UTC", 5, 10), { name: "UTC", transitions: [{ at: 5, offset: 0 }], covering: [5, 10] });
  assert.deepEqual(zoneTable("+05:30", 0, DAY).transitions, [{ at: 0, offset: 330 }]);
  assert.deepEqual(zoneTable("-0800", 0, DAY).transitions, [{ at: 0, offset: -480 }]);
});

test("transitions sorted strictly, the first at the span's start, offsets within ±18 h (zone.schema.json)", () => {
  for (const zone of ["Europe/London", "America/Santiago", "Asia/Tehran", "Pacific/Apia", "America/St_Johns"]) {
    const t = zoneTable(zone, Date.UTC(1990, 0, 1), Date.UTC(2030, 0, 1));
    assert.equal(t.transitions[0].at, t.covering[0], zone);
    for (let i = 1; i < t.transitions.length; i++) {
      assert.ok(t.transitions[i].at > t.transitions[i - 1].at, `${zone}: sorted`);
      assert.notEqual(t.transitions[i].offset, t.transitions[i - 1].offset, `${zone}: each a change`);
    }
    assert.ok(t.transitions.every((x) => Number.isInteger(x.at) && Number.isInteger(x.offset) && Math.abs(x.offset) <= 1080), zone);
  }
  // Samoa skipped 30 December 2011, from −10:00 to +14:00.
  assert.ok(zoneTable("Pacific/Apia", Date.UTC(2011, 11, 1), Date.UTC(2012, 0, 31)).transitions.some((x) => x.offset === 840));
});

test("every instant's table offset is Intl's", () => {
  const t = zoneTable("Europe/Berlin", Date.UTC(2025, 0, 1), Date.UTC(2027, 0, 1));
  const at = (ms) => t.transitions.filter((x) => x.at <= ms).at(-1).offset;
  for (let ms = t.covering[0]; ms <= t.covering[1]; ms += 0.37 * DAY) assert.equal(at(ms), offsetAt("Europe/Berlin", ms), new Date(ms).toISOString());
});

test("resolveZone: UTC, a fixed offset, an IANA name as written, the page's when empty, null when unknown", () => {
  assert.equal(resolveZone("UTC"), "UTC");
  assert.equal(resolveZone("utc"), "UTC");
  assert.equal(resolveZone("+05:30"), "+05:30");
  assert.equal(resolveZone("America/New_York"), "America/New_York");
  assert.equal(resolveZone(null), pageZone());
  assert.equal(resolveZone(""), new Intl.DateTimeFormat().resolvedOptions().timeZone);
  assert.equal(resolveZone("Mars/Olympus_Mons"), null);
  assert.deepEqual(["+05:30", "-08:00", "+0530", "+05", "+19:00", "+05:60", "5:30"].map(fixedOffset), [330, -480, 330, 300, null, null, null]);
});

test("a table covers a span; it is built over the span widened by its width, at least a day, each side", () => {
  assert.deepEqual(coveringFor(10 * DAY, 13 * DAY), [7 * DAY, 16 * DAY]);
  assert.deepEqual(coveringFor(0, 1000), [-DAY, DAY + 1000]);
  const t = zoneTable("UTC", 0, 100);
  assert.equal(covers(t, 0, 100), true);
  assert.equal(covers(t, -1, 50), false);
  assert.equal(covers(null, 0, 0), false);
});
