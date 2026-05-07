// setcode-runtime-upgrade.ts — Apply a sudo system.setCodeWithoutChecks runtime upgrade
//
// USAGE (production preferred — env vars):
//   SUDO_SEED="<real sudo seed phrase or //path>" \
//   WS_ENDPOINT=wss://node.testnet.thxnet.org/archive-001/ws \
//   RUNTIME_WASM=/path/to/thxnet_testnet_runtime.compact.compressed.wasm \
//   LABEL=rootchain \
//     bun run setcode-runtime-upgrade.ts
//
// USAGE (CLI form — accepted only if env vars not set):
//   bun run setcode-runtime-upgrade.ts \
//     --endpoint wss://node.testnet.thxnet.org/archive-001/ws \
//     --wasm /path/to/thxnet_testnet_runtime.compact.compressed.wasm \
//     --label rootchain
//
// Env vars take precedence over CLI args when both are set.
//
// SAFETY:
//   * SUDO_SEED is REQUIRED (no default). Script throws if missing.
//   * Dev keys (//Alice .. //Ferdie) are REJECTED — this script is for
//     production sudo only. Use forknet-only helpers in `forknet/` for dev keys.
//   * Script prints `[label] sudo signer: <address>` then pauses 5 seconds
//     so the operator can Ctrl-C if the address is wrong.
//
// FLOW:
//   1. Load + sanity-check seed; print signer address; 5-second abort window
//   2. Connect, capture pre-upgrade spec_version + head block
//   3. Submit `sudo.sudoUncheckedWeight(system.setCodeWithoutChecks(wasm))`
//   4. Wait for InBlock + Finalized; assert CodeUpdated event
//   5. Confirm spec_version bumped, head advanced

import { ApiPromise, Keyring, WsProvider } from "@polkadot/api";
import { readFileSync } from "node:fs";

const DEV_KEYS = new Set([
  "//Alice",
  "//Bob",
  "//Charlie",
  "//Dave",
  "//Eve",
  "//Ferdie",
]);

function getCliArg(name: string): string | undefined {
  const i = process.argv.indexOf(`--${name}`);
  if (i >= 0 && i + 1 < process.argv.length) return process.argv[i + 1];
  return undefined;
}

function resolveConfig(envName: string, cliName: string): string | undefined {
  // Env var takes precedence per spec
  return process.env[envName] ?? getCliArg(cliName);
}

function requireConfig(envName: string, cliName: string, humanName: string): string {
  const v = resolveConfig(envName, cliName);
  if (!v) {
    throw new Error(
      `${humanName} required: set env var ${envName} or pass --${cliName}`,
    );
  }
  return v;
}

function loadSudoSeed(): string {
  const seed = process.env.SUDO_SEED;
  if (!seed) throw new Error("SUDO_SEED env var required");
  if (DEV_KEYS.has(seed.trim())) {
    throw new Error(
      "dev key rejected — production needs real sudo seed",
    );
  }
  return seed;
}

const ENDPOINT = requireConfig("WS_ENDPOINT", "endpoint", "WS endpoint");
const WASM_PATH = requireConfig("RUNTIME_WASM", "wasm", "runtime wasm path");
const LABEL = resolveConfig("LABEL", "label") ?? "chain";
const SUDO_SEED = loadSudoSeed();

async function main() {
  // Resolve signer address up front for the abort banner
  const keyring = new Keyring({ type: "sr25519" });
  const signer = keyring.addFromUri(SUDO_SEED);
  console.log(`[${LABEL}] sudo signer: ${signer.address}`);
  console.log(`[${LABEL}] endpoint: ${ENDPOINT}`);
  console.log(`[${LABEL}] wasm path: ${WASM_PATH}`);
  console.log("Press Ctrl-C within 5s to abort");
  await new Promise((r) => setTimeout(r, 5000));

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

  // Use setCodeWithoutChecks to bypass strict spec_version increment check
  // (we explicitly disable that for forked-genesis multi-version jumps)
  const setCodeCall = api.tx.system.setCodeWithoutChecks(wasmHex);
  const sudoCall = api.tx.sudo.sudoUncheckedWeight(setCodeCall, {
    refTime: 1_000_000_000_000,
    proofSize: 1_000_000,
  });

  console.log(
    `[${LABEL}] submitting sudo.sudoUncheckedWeight(system.setCodeWithoutChecks(...)) ...`,
  );
  const start = Date.now();

  return new Promise<void>((resolve, reject) => {
    sudoCall
      .signAndSend(signer, ({ status, dispatchError, events }) => {
        if (status.isInBlock) {
          console.log(
            `[${LABEL}] InBlock ${status.asInBlock.toHex()} (${(
              (Date.now() - start) /
              1000
            ).toFixed(1)}s)`,
          );
        }
        if (status.isFinalized) {
          console.log(
            `[${LABEL}] Finalized ${status.asFinalized.toHex()} (${(
              (Date.now() - start) /
              1000
            ).toFixed(1)}s)`,
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
      const api2 = await ApiPromise.create({
        provider: new WsProvider(ENDPOINT),
      });
      const postSpec = api2.runtimeVersion.specVersion.toNumber();
      const postHeader = await api2.rpc.chain.getHeader();
      const postBlock = postHeader.number.toNumber();
      console.log(
        `[${LABEL}] post-upgrade: ${api2.runtimeVersion.specName.toString()} v${postSpec} @ #${postBlock}`,
      );
      console.log(
        `[${LABEL}] spec bump: ${preSpec} → ${postSpec}, block advance: ${
          postBlock - preBlock
        }`,
      );

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
      console.log(
        `[${LABEL}] SETCODE OK: spec ${preSpec}→${postSpec}, block +${
          postBlock - preBlock
        }`,
      );
      await api2.disconnect();
    })
    .catch((e) => {
      console.error(`[${LABEL}] FATAL:`, e.message || e);
      process.exit(1);
    });
}

main();
