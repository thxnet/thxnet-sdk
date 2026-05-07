// setcode-parachain.ts — Apply a cumulus 2-step parachain runtime upgrade
//
// Cumulus parachains REJECT direct system.setCode because the WASM blob
// would exhaust the block / PoV limits in a single extrinsic. Instead use:
//   1. sudo(parachainSystem.authorizeUpgrade(blake2_256(code), checkVersion=false))
//   2. parachainSystem.enactAuthorizedUpgrade(code)  -- unsigned, validated against authorized hash
//
// USAGE (production preferred — env vars):
//   SUDO_SEED="<real sudo seed phrase or //path>" \
//   WS_ENDPOINT=wss://node.<leaf>.testnet.thxnet.org/archive-001/ws \
//   RUNTIME_WASM=/path/to/general_runtime.compact.compressed.wasm \
//   LABEL=leafchain \
//     bun run setcode-parachain.ts
//
// USAGE (CLI form — accepted only if env vars not set):
//   bun run setcode-parachain.ts \
//     --endpoint wss://node.<leaf>.testnet.thxnet.org/archive-001/ws \
//     --wasm /path/to/general_runtime.compact.compressed.wasm \
//     --label leafchain
//
// Env vars take precedence over CLI args when both are set.
//
// SAFETY:
//   * SUDO_SEED is REQUIRED (no default). Script throws if missing.
//   * Dev keys (//Alice .. //Ferdie) are REJECTED — this script is for
//     production sudo only. Use forknet-only helpers in `forknet/` for dev keys.
//   * Script prints `[label] sudo signer: <address>` then pauses 5 seconds
//     so the operator can Ctrl-C if the address is wrong.

import { ApiPromise, Keyring, WsProvider } from "@polkadot/api";
import { blake2AsHex } from "@polkadot/util-crypto";
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
const LABEL = resolveConfig("LABEL", "label") ?? "para";
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
  const codeHash = blake2AsHex(wasmBuf, 256);
  console.log(`[${LABEL}] wasm: ${wasmBuf.length} bytes, hash=${codeHash}`);

  // Step 1: authorize upgrade by hash (small tx, fits easily)
  // parachainSystem.authorizeUpgrade(code_hash, check_version: bool)
  const authorize = api.tx.parachainSystem.authorizeUpgrade(codeHash, false);
  const sudoAuth = api.tx.sudo.sudo(authorize);

  console.log(
    `[${LABEL}] [1/2] submitting sudo(parachainSystem.authorizeUpgrade) ...`,
  );
  await new Promise<void>((resolve, reject) => {
    sudoAuth
      .signAndSend(signer, ({ status, dispatchError }) => {
        if (status.isInBlock) {
          if (dispatchError) {
            reject(
              new Error(`authorize dispatch error: ${dispatchError.toString()}`),
            );
            return;
          }
          console.log(
            `[${LABEL}] [1/2] authorized in ${status.asInBlock.toHex()}`,
          );
          resolve();
        }
      })
      .catch(reject);
  });

  // Wait one block for authorize to be on-chain finalized
  await new Promise((r) => setTimeout(r, 6_000));

  // Step 2: enact upgrade (the WASM blob, unsigned because it's validated against authorized hash)
  // Cumulus only allows ONE authorized upgrade at a time, and it's removed after enacting.
  const enact = api.tx.parachainSystem.enactAuthorizedUpgrade(wasmHex);

  console.log(
    `[${LABEL}] [2/2] submitting parachainSystem.enactAuthorizedUpgrade (unsigned) ...`,
  );
  const start = Date.now();
  await new Promise<void>((resolve, reject) => {
    enact
      .send(({ status, dispatchError }) => {
        if (status.isInBlock) {
          if (dispatchError) {
            reject(new Error(`enact dispatch error: ${dispatchError.toString()}`));
            return;
          }
          console.log(
            `[${LABEL}] [2/2] InBlock ${status.asInBlock.toHex()} (${(
              (Date.now() - start) /
              1000
            ).toFixed(1)}s)`,
          );
          resolve();
        }
      })
      .catch(reject);
  });

  // Cumulus parachain runtime upgrade requires a relay-chain block to advance
  // before the new runtime is actually applied. Wait ~24-36s.
  console.log(
    `[${LABEL}] waiting 36s for parachain to ingest relay block + apply new runtime ...`,
  );
  await new Promise((r) => setTimeout(r, 36_000));

  await api.disconnect();
  const api2 = await ApiPromise.create({ provider: new WsProvider(ENDPOINT) });
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
    `[${LABEL}] PARACHAIN SETCODE OK: spec ${preSpec}→${postSpec}, block +${
      postBlock - preBlock
    }`,
  );
  await api2.disconnect();
}

main().catch((err) => {
  console.error("FATAL:", err.message || err);
  process.exit(1);
});
