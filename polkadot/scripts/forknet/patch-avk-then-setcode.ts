// patch-avk-then-setcode.ts — Path B helper: patch ParasShared::ActiveValidatorKeys
// post-genesis, then issue `setCodeWithoutChecks` in the same block.
//
// Used during forknet rehearsal Path B to inject 6 active validators into a
// 2-validator-genesis snapshot before the relay parachain backing pipeline
// initialises. Empirically the genesis patch SURVIVES session 0 init (verified
// during rehearsal Path B); the runtime-side re-patch here covers cases where
// only a post-genesis touch-up is desired.
//
// Usage:
//   bun run patch-avk-then-setcode.ts \
//     --endpoint ws://localhost:9931 \
//     --avk-value 0x3c<...15 entries...> \
//     --wasm /path/to/thxnet_testnet_runtime.wasm

import { ApiPromise, Keyring, WsProvider } from "@polkadot/api";
import { readFileSync } from "node:fs";

// twox_128("ParasShared") ++ twox_128("ActiveValidatorKeys")
// = 0xb341e3a63e58a188839b242d17f8c9f8 ++ 0x7a50c904b368210021127f9238883a6e
// (empirically verified during rehearsal Path B; the earlier guess
// 0x5f3e4907... was actually the Staking pallet's storage key.)
const AVK_KEY = "0xb341e3a63e58a188839b242d17f8c9f87a50c904b368210021127f9238883a6e";

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
