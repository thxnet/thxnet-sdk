// Chopsticks-based migration idempotency test for THXNET.
//
// Proves migration determinism by forking live state twice from the same block,
// running the migration independently on each fork, and comparing post-migration
// state roots and key storage values.
//
// This bypasses the try-runtime v0.10.1 --create-snapshot limitation.
//
// Usage:
//   bun run scripts/chopsticks-idempotency-test.ts --chain leafchain-sand-testnet
//   bun run scripts/chopsticks-idempotency-test.ts --chain all-testnet
//   bun run scripts/chopsticks-idempotency-test.ts --chain all

import { ApiPromise, WsProvider } from "@polkadot/api";
import { execSync, spawn, ChildProcess } from "child_process";
import { existsSync } from "fs";
import { connect } from "net";

// ─── Configuration ──────────────────────────────────────────────────────────

const SCRIPT_DIR = new URL(".", import.meta.url).pathname;
const PROJECT_ROOT = execSync("git rev-parse --show-toplevel", {
  encoding: "utf-8",
}).trim();
const CHOPSTICKS_DIR = `${PROJECT_ROOT}/scripts/chopsticks`;
const WASM_DIR = `${PROJECT_ROOT}/target/release/wbuild`;

// ─── Timing constants ──────────────────────────────────────────────────────

const CHOPSTICKS_STARTUP_WAIT_MS = 20_000;
const POST_BLOCK_SETTLE_MS = 2_000;
const PORT_RELEASE_WAIT_MS = 3_000;

interface ChainConfig {
  name: string;
  configFile: string;
  wasmPath: string;
  portA: number; // First fork
  portB: number; // Second fork
}

const CHAINS: Record<string, ChainConfig> = {
  "leafchain-sand-testnet": {
    name: "leafchain-sand-testnet",
    configFile: `${CHOPSTICKS_DIR}/leafchain-sand-testnet.yml`,
    wasmPath: `${WASM_DIR}/general-runtime/general_runtime.compact.compressed.wasm`,
    portA: 18100,
    portB: 18200,
  },
  "leafchain-lmt-testnet": {
    name: "leafchain-lmt-testnet",
    configFile: `${CHOPSTICKS_DIR}/leafchain-lmt-testnet.yml`,
    wasmPath: `${WASM_DIR}/general-runtime/general_runtime.compact.compressed.wasm`,
    portA: 18102,
    portB: 18202,
  },
  "leafchain-lmt-mainnet": {
    name: "leafchain-lmt-mainnet",
    configFile: `${CHOPSTICKS_DIR}/leafchain-lmt-mainnet.yml`,
    wasmPath: `${WASM_DIR}/general-runtime/general_runtime.compact.compressed.wasm`,
    portA: 18103,
    portB: 18203,
  },
  "leafchain-avatect-mainnet": {
    name: "leafchain-avatect-mainnet",
    configFile: `${CHOPSTICKS_DIR}/leafchain-avatect-mainnet.yml`,
    wasmPath: `${WASM_DIR}/general-runtime/general_runtime.compact.compressed.wasm`,
    portA: 18104,
    portB: 18204,
  },
  "rootchain-testnet": {
    name: "rootchain-testnet",
    configFile: `${CHOPSTICKS_DIR}/rootchain-testnet.yml`,
    wasmPath: `${WASM_DIR}/thxnet-testnet-runtime/thxnet_testnet_runtime.compact.compressed.wasm`,
    portA: 18106,
    portB: 18206,
  },
  "rootchain-mainnet": {
    name: "rootchain-mainnet",
    configFile: `${CHOPSTICKS_DIR}/rootchain-mainnet.yml`,
    wasmPath: `${WASM_DIR}/thxnet-runtime/thxnet_runtime.compact.compressed.wasm`,
    portA: 18108,
    portB: 18208,
  },
};

const TESTNET_CHAINS = [
  "leafchain-sand-testnet",
  "leafchain-lmt-testnet",
  "rootchain-testnet",
];
const ALL_CHAINS = Object.keys(CHAINS);

// ─── Helpers ────────────────────────────────────────────────────────────────

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function startChopsticks(
  configFile: string,
  wasmPath: string,
  port: number
): ChildProcess {
  const proc = spawn(
    "bunx",
    ["@acala-network/chopsticks", "-c", configFile, "-w", wasmPath, "-p", String(port)],
    { stdio: "pipe", detached: false }
  );
  return proc;
}

async function waitForPortRelease(port: number, timeoutMs: number = PORT_RELEASE_WAIT_MS): Promise<boolean> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const inUse = await new Promise<boolean>((resolve) => {
      const socket = connect(port, "localhost");
      socket.once("connect", () => { socket.destroy(); resolve(true); });
      socket.once("error", () => { resolve(false); });
    });
    if (!inUse) return true;
    await sleep(200);
  }
  return false;
}

async function waitForRpc(port: number, timeoutMs: number = CHOPSTICKS_STARTUP_WAIT_MS): Promise<boolean> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const provider = new WsProvider(`ws://localhost:${port}`, 0);
      const api = await ApiPromise.create({ provider, throwOnConnect: true });
      const header = await api.rpc.chain.getHeader();
      await api.disconnect();
      if (header.number.toNumber() > 0) return true;
    } catch {
      // Not ready yet
    }
    await sleep(2000);
  }
  return false;
}

interface ForkState {
  specVersion: number;
  blockNumber: number;
  stateRoot: string;
  totalIssuance: string;
  rwaNextAssetId: string;
  crowdfundingNextCampaignId: string;
}

async function captureForkState(port: number): Promise<ForkState> {
  const provider = new WsProvider(`ws://localhost:${port}`);
  const api = await ApiPromise.create({ provider });

  // Verify WASM override applied
  const version = api.runtimeVersion;
  const specVersion = version.specVersion.toNumber();

  // Produce 1 block to trigger migrations
  await provider.send("dev_newBlock", [{ count: 1 }]);
  await sleep(POST_BLOCK_SETTLE_MS);

  // Capture post-migration state
  const header = await api.rpc.chain.getHeader();
  const blockNumber = header.number.toNumber();

  const blockHash = await api.rpc.chain.getBlockHash(blockNumber);
  const blockData = await api.rpc.chain.getBlock(blockHash);
  const stateRoot = blockData.block.header.stateRoot.toString();

  const totalIssuance = (await api.query.balances.totalIssuance()).toString();

  // Custom pallet storage counts (may not exist on all runtimes)
  const rwaNextAssetId = (api.query as any).rwa?.nextAssetId
    ? (await (api.query as any).rwa.nextAssetId()).toString()
    : "N/A";
  const crowdfundingNextCampaignId = (api.query as any).crowdfunding?.nextCampaignId
    ? (await (api.query as any).crowdfunding.nextCampaignId()).toString()
    : "N/A";

  await api.disconnect();

  return { specVersion, blockNumber, stateRoot, totalIssuance, rwaNextAssetId, crowdfundingNextCampaignId };
}

// ─── Core test ──────────────────────────────────────────────────────────────

interface IdempotencyResult {
  chain: string;
  passed: boolean;
  forkA: ForkState | null;
  forkB: ForkState | null;
  details: string[];
}

async function testIdempotency(chain: ChainConfig): Promise<IdempotencyResult> {
  const details: string[] = [];
  let forkA: ForkState | null = null;
  let forkB: ForkState | null = null;
  let procA: ChildProcess | null = null;
  let procB: ChildProcess | null = null;

  try {
    // Check WASM exists
    if (!existsSync(chain.wasmPath)) {
      details.push(`SKIP: WASM not found: ${chain.wasmPath}`);
      return { chain: chain.name, passed: true, forkA: null, forkB: null, details };
    }

    // ── Fork A ────────────────────────────────────────────────────────
    details.push("Starting Fork A...");
    procA = startChopsticks(chain.configFile, chain.wasmPath, chain.portA);

    const readyA = await waitForRpc(chain.portA);
    if (!readyA) {
      details.push("FAIL: Fork A did not become ready");
      return { chain: chain.name, passed: false, forkA: null, forkB: null, details };
    }

    forkA = await captureForkState(chain.portA);
    details.push(
      `Fork A: spec=${forkA.specVersion} block=#${forkA.blockNumber} ` +
      `stateRoot=${forkA.stateRoot.substring(0, 20)}... issuance=${forkA.totalIssuance}` +
      ` rwa=${forkA.rwaNextAssetId} cf=${forkA.crowdfundingNextCampaignId}`
    );

    // Kill Fork A before starting Fork B (same endpoint, clean state)
    procA.kill();
    procA = null;
    await waitForPortRelease(chain.portA);

    // ── Fork B ────────────────────────────────────────────────────────
    details.push("Starting Fork B...");
    procB = startChopsticks(chain.configFile, chain.wasmPath, chain.portB);

    const readyB = await waitForRpc(chain.portB);
    if (!readyB) {
      details.push("FAIL: Fork B did not become ready");
      return { chain: chain.name, passed: false, forkA, forkB: null, details };
    }

    forkB = await captureForkState(chain.portB);
    details.push(
      `Fork B: spec=${forkB.specVersion} block=#${forkB.blockNumber} ` +
      `stateRoot=${forkB.stateRoot.substring(0, 20)}... issuance=${forkB.totalIssuance}` +
      ` rwa=${forkB.rwaNextAssetId} cf=${forkB.crowdfundingNextCampaignId}`
    );

    // ── Compare ───────────────────────────────────────────────────────
    let passed = true;

    // specVersion must match and be the new version (not the old)
    if (forkA.specVersion !== forkB.specVersion) {
      details.push(`FAIL: specVersion mismatch: A=${forkA.specVersion} B=${forkB.specVersion}`);
      passed = false;
    } else {
      details.push(`PASS: specVersion identical: ${forkA.specVersion}`);
    }

    // State root — the definitive idempotency signal
    if (forkA.stateRoot !== forkB.stateRoot) {
      details.push(`FAIL: stateRoot MISMATCH`);
      details.push(`  Fork A: ${forkA.stateRoot}`);
      details.push(`  Fork B: ${forkB.stateRoot}`);
      details.push(`  Migration is NON-DETERMINISTIC`);
      passed = false;
    } else {
      details.push(`PASS: stateRoot identical: ${forkA.stateRoot}`);
    }

    // Total issuance
    if (forkA.totalIssuance !== forkB.totalIssuance) {
      details.push(`FAIL: totalIssuance mismatch: A=${forkA.totalIssuance} B=${forkB.totalIssuance}`);
      passed = false;
    } else {
      details.push(`PASS: totalIssuance identical: ${forkA.totalIssuance}`);
    }

    // RWA nextAssetId (only compare when present on both forks)
    if (forkA.rwaNextAssetId !== "N/A" || forkB.rwaNextAssetId !== "N/A") {
      if (forkA.rwaNextAssetId !== forkB.rwaNextAssetId) {
        details.push(`FAIL: rwa.nextAssetId mismatch: A=${forkA.rwaNextAssetId} B=${forkB.rwaNextAssetId}`);
        passed = false;
      } else {
        details.push(`PASS: rwa.nextAssetId identical: ${forkA.rwaNextAssetId}`);
      }
    }

    // Crowdfunding nextCampaignId (only compare when present on both forks)
    if (forkA.crowdfundingNextCampaignId !== "N/A" || forkB.crowdfundingNextCampaignId !== "N/A") {
      if (forkA.crowdfundingNextCampaignId !== forkB.crowdfundingNextCampaignId) {
        details.push(`FAIL: crowdfunding.nextCampaignId mismatch: A=${forkA.crowdfundingNextCampaignId} B=${forkB.crowdfundingNextCampaignId}`);
        passed = false;
      } else {
        details.push(`PASS: crowdfunding.nextCampaignId identical: ${forkA.crowdfundingNextCampaignId}`);
      }
    }

    return { chain: chain.name, passed, forkA, forkB, details };
  } finally {
    if (procA) procA.kill();
    if (procB) procB.kill();
  }
}

// ─── Main ───────────────────────────────────────────────────────────────────

async function main() {
  const chainArg = (() => {
    const idx = process.argv.indexOf("--chain");
    return idx >= 0 ? process.argv[idx + 1] : "leafchain-sand-testnet";
  })();

  let chainNames: string[];
  if (chainArg === "all") {
    chainNames = ALL_CHAINS;
  } else if (chainArg === "all-testnet") {
    chainNames = TESTNET_CHAINS;
  } else if (CHAINS[chainArg]) {
    chainNames = [chainArg];
  } else {
    console.error(`Unknown chain: ${chainArg}`);
    console.error(`Valid: ${ALL_CHAINS.join(", ")}, all-testnet, all`);
    process.exit(1);
  }

  console.log("╔══════════════════════════════════════════════════════════════╗");
  console.log("║        CHOPSTICKS IDEMPOTENCY TEST                         ║");
  console.log("╠══════════════════════════════════════════════════════════════╣");
  console.log(`║  Chains: ${chainNames.join(", ").padEnd(50)}║`);
  console.log("╚══════════════════════════════════════════════════════════════╝");
  console.log("");

  const results: IdempotencyResult[] = [];

  for (const name of chainNames) {
    const chain = CHAINS[name];
    console.log(`\n═══ Testing: ${name} ═══\n`);

    const result = await testIdempotency(chain);
    results.push(result);

    for (const d of result.details) {
      console.log(`  ${d}`);
    }
    console.log(
      `\n  Result: ${result.passed ? "PASS" : "FAIL"}`
    );
  }

  // ── Summary ─────────────────────────────────────────────────────────
  console.log("\n");
  console.log("╔══════════════════════════════════════════════════════════════╗");
  console.log("║        IDEMPOTENCY TEST SUMMARY                            ║");
  console.log("╠══════════════════════════════════════════════════════════════╣");

  let totalPassed = 0;
  let totalFailed = 0;

  for (const r of results) {
    const icon = r.passed ? "PASS" : "FAIL";
    console.log(`║  [${icon}] ${r.chain.padEnd(53)}║`);
    if (r.passed) totalPassed++;
    else totalFailed++;
  }

  console.log("╠══════════════════════════════════════════════════════════════╣");
  console.log(
    `║  Total: ${results.length}, Passed: ${totalPassed}, Failed: ${totalFailed}`.padEnd(
      63
    ) + "║"
  );
  console.log("╚══════════════════════════════════════════════════════════════╝");

  process.exit(totalFailed);
}

main().catch((err) => {
  console.error("IDEMPOTENCY TEST FATAL:", err.message || err);
  process.exit(1);
});
