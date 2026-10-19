// Sums twiggy's `top --format csv` rows (stdin) by owner, by name.
let input = "";
process.stdin.on("data", (d) => (input += d)).on("end", () => {
  const rules = [
    ["glyphplot bench code (ladder::)", /ladder::|LineChart|CandleChart|metas_json|^version/],
    ["serde, serde_json", /serde|ryu|itoa|zmij/],
    ["wasm-bindgen", /wasm_bindgen|__wbindgen|__wbg|externref/],
    ["core::fmt (formatting, panics)", /core::fmt|core::panicking|core::result::unwrap_failed|core::option::expect_failed|Display|Debug|fmt::/],
    ["float formatting and parsing", /flt2dec|dec2flt|float::|lexical|grisu|dragon/i],
    ["allocator (dlmalloc)", /dlmalloc|__rust_alloc|__rust_dealloc|__rust_realloc|alloc::alloc/],
    ["libm (log10, sin, floor...)", /compiler_builtins|libm|log10|^floor|^ceil|^pow|^sin|^exp/],
    ["rest of std, core, alloc", /std::|core::|alloc::|rust_begin_unwind|__rust/],
    ["data section", /data\[|\.rodata/],
  ];
  const sums = new Map();
  let total = 0;
  for (const line of input.split("\n").slice(1)) {
    const m = line.match(/^"?(.*?)"?,(\d+),/);
    if (!m) continue;
    const bytes = m[2];
    const name = m[1].replace(/\[[0-9a-f]{16}\]/g, "");
    if (/custom section|function names|subsection/.test(name)) continue;
    const n = Number(bytes);
    total += n;
    const owner = rules.find(([, re]) => re.test(name))?.[0] ?? "other (tables, type, names, unnamed)";
    sums.set(owner, (sums.get(owner) ?? 0) + n);
  }
  for (const [k, v] of [...sums].sort((a, b) => b[1] - a[1])) console.log(`  ${k.padEnd(40)} ${String(v).padStart(8)} B ${((100 * v) / total).toFixed(1).padStart(5)}%`);
});
