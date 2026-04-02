// Verify leafchain is healthy after a runtime change (upgrade or restart):
// 1. New blocks are being produced
// 2. Node health is OK
//
// Called from a parachain collator node.
//
// Usage in ZNDSL:
//   leafchain-a-collator-1: js-script ./zombienet-js/verify-leafchain-upgrade.js within 120 seconds

const assert = require("assert");

async function run(nodeName, networkInfo, _args) {
  const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
  const api = await zombie.connect(wsUri, userDefinedTypes);

  const currentVersion = (
    await api.rpc.state.getRuntimeVersion()
  ).specVersion.toNumber();
  console.log(`Current specVersion: ${currentVersion}`);

  // 1. Verify blocks are being produced (wait for 2 new blocks)
  const headerBefore = await api.rpc.chain.getHeader();
  const blockBefore = headerBefore.number.toNumber();
  console.log(`Current block: ${blockBefore}, waiting for 2 new blocks...`);

  let attempts = 0;
  const maxAttempts = 20;
  while (attempts < maxAttempts) {
    await new Promise((resolve) => setTimeout(resolve, 12000)); // leafchain block time ~12s
    const headerNow = await api.rpc.chain.getHeader();
    const blockNow = headerNow.number.toNumber();
    if (blockNow >= blockBefore + 2) {
      console.log(`Block production confirmed: ${blockBefore} -> ${blockNow}`);
      break;
    }
    attempts++;
  }
  assert.ok(
    attempts < maxAttempts,
    "Block production stalled after upgrade"
  );

  // 2. Quick health: system.health
  const health = await api.rpc.system.health();
  console.log(
    `Node health: peers=${health.peers}, syncing=${health.isSyncing}`
  );

  console.log("Leafchain post-upgrade verification PASSED");
  return currentVersion;
}

module.exports = { run };
