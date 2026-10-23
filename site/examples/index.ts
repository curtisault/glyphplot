// Every example, in gallery order (REQ-SITE-040): each module is
// site/examples/<slug>.ts exporting `example`; manifest.json holds their
// metadata in the same order. The first two are the live candlesticks
// REQ-SITE-040 and ADR-136 put first. Importing this loads no wasm: an
// example's data is made only when its `start()` is called.
import type { Example } from "./api.ts";
import { example as liveMarket } from "./live-market.ts";
import { example as wikimediaEdits } from "./wikimedia-edits.ts";
import { example as helloLine } from "./hello-line.ts";
import { example as millionPoints } from "./million-points.ts";
import { example as telemetryGaps } from "./telemetry-gaps.ts";
import { example as thermostat } from "./thermostat.ts";
import { example as cpuArea } from "./cpu-area.ts";
import { example as oilYear } from "./oil-year.ts";
import { example as tenYearYield } from "./ten-year-yield.ts";
import { example as euroDollar } from "./euro-dollar.ts";
import { example as latencyPercentiles } from "./latency-percentiles.ts";
import { example as netFlow } from "./net-flow.ts";
import { example as deploys } from "./deploys.ts";
import { example as sloBand } from "./slo-band.ts";
import { example as twoUnits } from "./two-units.ts";
import { example as threeZones } from "./three-zones.ts";
import { example as locales } from "./locales.ts";
import { example as tideGauge } from "./tide-gauge.ts";
import { example as ecgStrip } from "./ecg-strip.ts";
import { example as keyboardOnly } from "./keyboard-only.ts";
import { example as screenReader } from "./screen-reader.ts";
import { example as oneInk } from "./one-ink.ts";
import { example as onPaper } from "./on-paper.ts";
import { example as cssThemes } from "./css-themes.ts";
import { example as brandTheme } from "./brand-theme.ts";
import { example as sizes } from "./sizes.ts";
import { example as viewInUrl } from "./view-in-url.ts";
import { example as linkedCharts } from "./linked-charts.ts";
import { example as brushZoom } from "./brush-zoom.ts";
import { example as everyEvent } from "./every-event.ts";
import { example as csvSrc } from "./csv-src.ts";
import { example as arrowMillion } from "./arrow-million.ts";
import { example as typedArrays } from "./typed-arrays.ts";
import { example as plainHtml } from "./plain-html.ts";
import { example as hoverEvent } from "./hover-event.ts";
import { example as reducedMotion } from "./reduced-motion.ts";
import { example as sparklineTable } from "./sparkline-table.ts";
import { example as sparklineSentence } from "./sparkline-sentence.ts";
import { example as upDownDays } from "./up-down-days.ts";
import { example as fiftySparklines } from "./fifty-sparklines.ts";
import { example as oilWeekly } from "./oil-weekly.ts";
import { example as hollowOneInk } from "./hollow-one-ink.ts";
import { example as ohlcBars } from "./ohlc-bars.ts";
import { example as yearsToMinutes } from "./years-to-minutes.ts";
import { example as sessionGaps } from "./session-gaps.ts";
import { example as lastPrice } from "./last-price.ts";
import { example as topTalkers } from "./top-talkers.ts";
import { example as budgetActual } from "./budget-actual.ts";
import { example as energyMix } from "./energy-mix.ts";
import { example as kIndex } from "./k-index.ts";
import { example as quakes } from "./quakes.ts";
import { example as loan } from "./loan.ts";
import { example as burnDown } from "./burn-down.ts";
import { example as telemetry1khz } from "./telemetry-1khz.ts";
import { example as lateDuplicate } from "./late-duplicate.ts";
import { example as lastHour } from "./last-hour.ts";
import { example as throughput } from "./throughput.ts";
import { example as serviceDashboard } from "./service-dashboard.ts";
import { example as yieldCurve } from "./yield-curve.ts";
import { example as yieldCurveYear } from "./yield-curve-year.ts";

export const examples: Example[] = [
  liveMarket,
  wikimediaEdits,
  helloLine,
  millionPoints,
  telemetryGaps,
  thermostat,
  cpuArea,
  oilYear,
  tenYearYield,
  euroDollar,
  latencyPercentiles,
  netFlow,
  deploys,
  sloBand,
  twoUnits,
  threeZones,
  locales,
  tideGauge,
  ecgStrip,
  keyboardOnly,
  screenReader,
  oneInk,
  onPaper,
  cssThemes,
  brandTheme,
  sizes,
  viewInUrl,
  linkedCharts,
  brushZoom,
  everyEvent,
  csvSrc,
  arrowMillion,
  typedArrays,
  plainHtml,
  hoverEvent,
  reducedMotion,
  sparklineTable,
  sparklineSentence,
  upDownDays,
  fiftySparklines,
  oilWeekly,
  hollowOneInk,
  ohlcBars,
  yearsToMinutes,
  sessionGaps,
  lastPrice,
  topTalkers,
  budgetActual,
  energyMix,
  kIndex,
  quakes,
  loan,
  burnDown,
  telemetry1khz,
  lateDuplicate,
  lastHour,
  throughput,
  serviceDashboard,
  yieldCurve,
  yieldCurveYear,
];

/** The example with this slug, or undefined. */
export function exampleOf(slug: string): Example | undefined {
  return examples.find((e) => e.slug === slug);
}
