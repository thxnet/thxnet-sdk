// inject-leaf-wasm-into-para-spec.ts
// Replace the ":code" storage entry in a para chain spec with a new WASM blob.
// Storage key for :code = 0x3a636f6465 (raw hex of ":code")
//
// Usage:
//   bun run inject-leaf-wasm-into-para-spec.ts \
//     --in w6-t3-verify.json \
//     --wasm general_runtime.compact.compressed.wasm \
//     --out w6-t3-verify-v1.12.0.json

import { readFileSync, writeFileSync } from "node:fs";

function arg(name: string, required = true): string {
  const i = process.argv.indexOf(`--${name}`);
  if (i >= 0 && i + 1 < process.argv.length) return process.argv[i + 1];
  if (required) throw new Error(`missing --${name}`);
  return "";
}

const IN = arg("in");
const WASM = arg("wasm");
const OUT = arg("out");

const CODE_KEY = "0x3a636f6465";

console.log(`reading para spec: ${IN}`);
const specRaw = readFileSync(IN, "utf8");
console.log(`  loaded: ${specRaw.length} bytes`);
const spec = JSON.parse(specRaw);

const top = spec?.genesis?.raw?.top;
if (!top) throw new Error("spec.genesis.raw.top not found");

const oldCode = top[CODE_KEY];
if (typeof oldCode !== "string") {
  throw new Error(`spec.genesis.raw.top['${CODE_KEY}'] not found or not a string`);
}
const oldCodeBytes = (oldCode.length - 2) / 2;
console.log(`  current :code length: ${oldCodeBytes} bytes (${oldCodeBytes / 1024 / 1024 | 0} MB-ish)`);

console.log(`reading wasm: ${WASM}`);
const wasmBuf = readFileSync(WASM);
const newCodeHex = "0x" + wasmBuf.toString("hex");
console.log(`  loaded: ${wasmBuf.length} bytes`);

top[CODE_KEY] = newCodeHex;

console.log(`writing patched spec: ${OUT}`);
writeFileSync(OUT, JSON.stringify(spec));
console.log(`  done. :code: ${oldCodeBytes} -> ${wasmBuf.length} bytes`);
