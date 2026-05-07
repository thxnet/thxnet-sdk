// patch-avk-then-setcode.ts — Path B helper: patch parachains_shared::ActiveValidatorKeys
// at runtime via sudo.system.setStorage, then trigger relay setCode.
//
// Goal: make migration's `decode_len()` see 15 (genesis patch gets reset by session init,
// so we must do this AT runtime within session 0).
//
// Usage:
//   bun run patch-avk-then-setcode.ts \
//     --endpoint ws://localhost:9931 \
//     --avk-value 0x3c<...15 entries...> \
//     --wasm /path/to/thxnet_testnet_runtime.wasm

import { ApiPromise, Keyring, WsProvider } from "@polkadot/api";
import { readFileSync } from "node:fs";

const AVK_KEY = "0x5f3e4907f716ac89b6347d15ececedca5579297f4dfb9609e7e4c2ebab9ce40a";

function getArg(name: string, fallback?: string): string {
  const i = process.argv.indexOf(`--${name}`);
  if (i >= 0 && i + 1 < process.argv.length) return process.argv[i + 1];
  if (fallback !== undefined) return fallback;
  throw new Error(`missing --${name}`);
}

async function main() {
  const endpoint = getArg("endpoint");
  const avkValue = getArg("avk-value");
  const wasmPath = getArg("wasm");

  console.log(`[patch-avk] connecting to ${endpoint}`);
  const api = await ApiPromise.create({ provider: new WsProvider(endpoint) });
  await api.isReady;

  const alice = new Keyring({ type: "sr25519" }).addFromUri("//Alice");

  console.log(`[patch-avk] step 1: query current AVK length`);
  const cur = await api.rpc.state.getStorage(AVK_KEY);
  console.log(`  current AVK raw len: ${cur.toHex().length / 2 - 1} bytes`);

  console.log(`[patch-avk] step 2: sudo(system.setStorage([(AVK, new_value)]))`);
  const setStorageCall = api.tx.system.setStorage([[AVK_KEY, avkValue]]);
  const sudoSet = api.tx.sudo.sudo(setStorageCall);
  await new Promise<void>((resolve, reject) => {
    sudoSet.signAndSend(alice, ({ status, dispatchError }) => {
      if (status.isInBlock) {
        if (dispatchError) reject(new Error(`set_storage err: ${dispatchError}`));
        else { console.log(`  set_storage InBlock ${status.asInBlock.toHex()}`); resolve(); }
      }
    }).catch(reject);
  });

  console.log(`[patch-avk] step 3: verify AVK now has new length`);
  await new Promise(r => setTimeout(r, 6500));
  const after = await api.rpc.state.getStorage(AVK_KEY);
  const newLen = after.toHex().length / 2 - 1;
  console.log(`  post-patch AVK raw len: ${newLen} bytes`);
  if (newLen < 200) {
    console.error("  FAIL: AVK didn't update; expected ≥ 481 bytes for 15 entries");
    process.exit(1);
  }

  console.log(`[patch-avk] step 4: sudo(system.setCodeWithoutChecks(wasm))`);
  const wasmHex = "0x" + readFileSync(wasmPath).toString("hex");
  const setCodeCall = api.tx.system.setCodeWithoutChecks(wasmHex);
  const sudoCode = api.tx.sudo.sudoUncheckedWeight(setCodeCall, { refTime: 0, proofSize: 0 });
  const start = Date.now();
  await new Promise<void>((resolve, reject) => {
    sudoCode.signAndSend(alice, ({ status, dispatchError }) => {
      if (status.isInBlock) {
        if (dispatchError) reject(new Error(`setCode err: ${dispatchError}`));
        else { console.log(`  setCode InBlock ${status.asInBlock.toHex()} (${((Date.now() - start) / 1000).toFixed(1)}s)`); resolve(); }
      }
    }).catch(reject);
  });

  console.log(`[patch-avk] step 5: wait 18s for migration to apply ...`);
  await new Promise(r => setTimeout(r, 18_000));

  await api.disconnect();
  const api2 = await ApiPromise.create({ provider: new WsProvider(endpoint) });
  const ver = api2.runtimeVersion;
  console.log(`  post-setCode runtime: ${ver.specName} v${ver.specVersion}`);
  await api2.disconnect();
}

main().catch(e => { console.error("FATAL:", e); process.exit(1); });
