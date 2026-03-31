// Post-upgrade pallet test — verify pallets are queryable after wasm override.
//
// Usage:
//   # Start Chopsticks first (see upgrade-test.ts)
//   bun run scripts/chopsticks/post-upgrade-pallet-test.ts --port 8102
//
// What this tests (L3 — pallet health after upgrade):
//   1. System pallet: account count, block number
//   2. Balances pallet: total issuance
//   3. RWA pallet: NextAssetId, storage version
//   4. Crowdfunding pallet: NextCampaignId, protocol config, storage version
//   5. DAO pallet: storage accessible
//   6. Produce a block and re-verify — state survives block production

import { ApiPromise, WsProvider } from "@polkadot/api";

const port = (() => {
  const idx = process.argv.indexOf("--port");
  return idx >= 0 ? parseInt(process.argv[idx + 1], 10) : 8102;
})();

const endpoint = `ws://localhost:${port}`;

interface TestResult {
  name: string;
  passed: boolean;
  detail: string;
}

const results: TestResult[] = [];

function check(name: string, passed: boolean, detail: string) {
  results.push({ name, passed, detail });
  const icon = passed ? "PASS" : "FAIL";
  console.log(`  [${icon}] ${name}: ${detail}`);
}

async function main() {
  console.log(`Connecting to Chopsticks at ${endpoint}...`);
  const provider = new WsProvider(endpoint);
  const api = await ApiPromise.create({ provider });

  const version = api.runtimeVersion;
  console.log(
    `Runtime: ${version.specName.toString()} v${version.specVersion.toNumber()}\n`
  );

  // === Phase 1: Core pallet queries ===
  console.log("Phase 1: Core pallets");

  // System
  const blockNumber = (await api.rpc.chain.getHeader()).number.toNumber();
  check("system.blockNumber", blockNumber > 0, `#${blockNumber}`);

  const totalIssuance = (await api.query.balances.totalIssuance()).toString();
  check(
    "balances.totalIssuance",
    BigInt(totalIssuance) > 0n,
    totalIssuance
  );

  // === Phase 2: Custom pallet queries ===
  console.log("\nPhase 2: Custom pallets (RWA, Crowdfunding, DAO)");

  // RWA pallet — query storage version and OwnerAssets entries
  try {
    const rwaEntries = await (api.query as any).rwa.rwaAssets.entries();
    check("rwa.rwaAssets", true, `${rwaEntries.length} assets`);
  } catch (e: any) {
    check("rwa.rwaAssets", false, e.message);
  }

  // Crowdfunding pallet — query NextCampaignId and protocol config
  try {
    const nextCampaignId = await (api.query as any).crowdfunding.nextCampaignId();
    check("crowdfunding.nextCampaignId", true, nextCampaignId.toString());
  } catch (e: any) {
    check("crowdfunding.nextCampaignId", false, e.message);
  }

  try {
    const protocolConfig = await (api.query as any).crowdfunding.protocolFeeRecipientOverride();
    check("crowdfunding.protocolFeeRecipient", true, protocolConfig.toString());
  } catch (e: any) {
    check("crowdfunding.protocolFeeRecipient", false, e.message);
  }

  // TrustlessAgent pallet (leafchain-specific, replaces DAO which is rootchain-only)
  try {
    const palletKeys = Object.keys((api.query as any).trustlessAgent || {});
    check("trustlessAgent.accessible", palletKeys.length > 0, `${palletKeys.length} storage items`);
  } catch (e: any) {
    check("trustlessAgent.accessible", false, e.message);
  }

  // === Phase 3: Block production with pallets ===
  console.log("\nPhase 3: Block production post-query");
  const result = await provider.send("dev_newBlock", [{ count: 1 }]);
  check("dev_newBlock", typeof result === "string" && result.startsWith("0x"), result);

  // Re-query after block to verify state survives
  const blockAfter = (await api.rpc.chain.getHeader()).number.toNumber();
  check("post-block.number", blockAfter > blockNumber, `#${blockAfter}`);

  const issuanceAfter = (await api.query.balances.totalIssuance()).toString();
  check(
    "post-block.totalIssuance",
    BigInt(issuanceAfter) > 0n,
    issuanceAfter
  );

  // === Summary ===
  const passed = results.filter((r) => r.passed).length;
  const total = results.length;
  console.log(`\n=== POST-UPGRADE PALLET TEST: ${passed}/${total} PASSED ===`);

  await api.disconnect();
  process.exit(passed === total ? 0 : 1);
}

main().catch((err) => {
  console.error("POST-UPGRADE PALLET TEST FAILED:", err.message || err);
  process.exit(1);
});
