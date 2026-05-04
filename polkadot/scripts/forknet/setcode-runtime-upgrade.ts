// setcode-runtime-upgrade.ts — Upgrade running forknet to release/v1.12.0 via sudo.setCode
//
// Usage:
//   bun run setcode-runtime-upgrade.ts \
//     --endpoint ws://localhost:9931 \
//     --wasm /path/to/thxnet_testnet_runtime.compact.compressed.wasm \
//     --label rootchain
//
// Flow:
//   1. Connect, capture pre-upgrade spec_version + head block
//   2. Submit `sudo(system.setCodeWithoutChecks(wasm))` signed by Alice
//   3. Wait for InBlock + Finalized
//   4. Confirm spec_version bumped, head advanced N more blocks

import { ApiPromise, Keyring, WsProvider } from "@polkadot/api";
import { readFileSync } from "node:fs";

function getArg(name: string, fallback?: string): string {
  const i = process.argv.indexOf(`--${name}`);
  if (i >= 0 && i + 1 < process.argv.length) return process.argv[i + 1];
  if (fallback !== undefined) return fallback;
  throw new Error(`missing arg --${name}`);
}

const ENDPOINT = getArg("endpoint");
const WASM_PATH = getArg("wasm");
const LABEL = getArg("label", "chain");

async function main() {
  console.log(`[${LABEL}] connecting to ${ENDPOINT}`);
  const api = await ApiPromise.create({ provider: new WsProvider(ENDPOINT) });
  await api.isReady;

  const preSpec = api.runtimeVersion.specVersion.toNumber();
  const preName = api.runtimeVersion.specName.toString();
  const preHeader = await api.rpc.chain.getHeader();
  const preBlock = preHeader.number.toNumber();
  console.log(`[${LABEL}] pre-upgrade: ${preName} v${preSpec} @ #${preBlock}`);

  const wasmBuf = readFileSync(WASM_PATH);
  const wasmHex = "0x" + wasmBuf.toString("hex");
  console.log(`[${LABEL}] wasm loaded: ${wasmBuf.length} bytes`);

  const keyring = new Keyring({ type: "sr25519" });
  const alice = keyring.addFromUri("//Alice");
  console.log(`[${LABEL}] sudo signer: ${alice.address}`);

  // Use setCodeWithoutChecks to bypass strict spec_version increment check
  // (we explicitly disable that for forked-genesis multi-version jumps)
  const setCodeCall = api.tx.system.setCodeWithoutChecks(wasmHex);
  const sudoCall = api.tx.sudo.sudoUncheckedWeight(setCodeCall, {
    refTime: 1_000_000_000_000,
    proofSize: 1_000_000,
  });

  console.log(`[${LABEL}] submitting sudo.sudoUncheckedWeight(system.setCodeWithoutChecks(...)) ...`);
  const start = Date.now();

  return new Promise<void>((resolve, reject) => {
    sudoCall
      .signAndSend(alice, ({ status, dispatchError, events }) => {
        if (status.isInBlock) {
          console.log(
            `[${LABEL}] InBlock ${status.asInBlock.toHex()} (${((Date.now() - start) / 1000).toFixed(1)}s)`
          );
        }
        if (status.isFinalized) {
          console.log(
            `[${LABEL}] Finalized ${status.asFinalized.toHex()} (${((Date.now() - start) / 1000).toFixed(1)}s)`
          );
          if (dispatchError) {
            const decoded = dispatchError.isModule
              ? api.registry.findMetaError(dispatchError.asModule)
              : dispatchError.toString();
            console.error(`[${LABEL}] DISPATCH ERROR:`, decoded);
            reject(new Error(`dispatch error: ${JSON.stringify(decoded)}`));
            return;
          }

          // Look for CodeUpdated event
          let codeUpdated = false;
          for (const { event } of events) {
            if (api.events.system?.CodeUpdated?.is(event)) {
              codeUpdated = true;
            }
          }
          console.log(`[${LABEL}] CodeUpdated event present: ${codeUpdated}`);
          resolve();
        }
      })
      .catch(reject);
  })
    .then(async () => {
      // Wait one more block for the new runtime to take effect
      console.log(`[${LABEL}] waiting 12s for new runtime to take effect ...`);
      await new Promise((r) => setTimeout(r, 12_000));

      // Reconnect to pick up new metadata
      await api.disconnect();
      const api2 = await ApiPromise.create({ provider: new WsProvider(ENDPOINT) });
      const postSpec = api2.runtimeVersion.specVersion.toNumber();
      const postHeader = await api2.rpc.chain.getHeader();
      const postBlock = postHeader.number.toNumber();
      console.log(`[${LABEL}] post-upgrade: ${api2.runtimeVersion.specName.toString()} v${postSpec} @ #${postBlock}`);
      console.log(`[${LABEL}] spec bump: ${preSpec} → ${postSpec}, block advance: ${postBlock - preBlock}`);

      if (postSpec === preSpec) {
        console.error(`[${LABEL}] FAIL: spec_version did not change`);
        await api2.disconnect();
        process.exit(1);
      }
      if (postBlock <= preBlock) {
        console.error(`[${LABEL}] FAIL: block did not advance`);
        await api2.disconnect();
        process.exit(1);
      }
      console.log(`[${LABEL}] SETCODE OK: spec ${preSpec}→${postSpec}, block +${postBlock - preBlock}`);
      await api2.disconnect();
    })
    .catch((e) => {
      console.error(`[${LABEL}] FATAL:`, e.message || e);
      process.exit(1);
    });
}

main();
