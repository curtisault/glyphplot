// Throwaway. Writes copies of a wasm module with a custom section of random
// (incompressible) bytes appended, so its gzip -9 size reaches each target in
// KB: the load measurement's stand-in for a bigger module. A custom section is
// downloaded but not compiled, so these time transfer, not compilation.
//   node pad.mjs ../pkg/6-downsample/ladder_bg.wasm 150 300 1000
import { readFileSync, writeFileSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { randomBytes } from "node:crypto";

const [src, ...targets] = process.argv.slice(2);
const base = readFileSync(src);
const leb = (n) => { const out = []; do { let b = n & 0x7f; n >>>= 7; if (n) b |= 0x80; out.push(b); } while (n); return Buffer.from(out); };
for (const kb of targets) {
  const gz0 = gzipSync(base, { level: 9 }).length;
  const pad = Math.max(0, Number(kb) * 1024 - gz0 - 64);
  const name = Buffer.from("pad");
  const payload = Buffer.concat([leb(name.length), name, randomBytes(pad)]);
  const out = Buffer.concat([base, Buffer.from([0]), leb(payload.length), payload]);
  const file = src.replace(/\.wasm$/, `_pad${kb}.wasm`);
  writeFileSync(file, out);
  console.log(`${file}: ${out.length} raw, ${gzipSync(out, { level: 9 }).length} gzip -9`);
}
