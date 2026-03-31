// Verify that a runtime upgrade has occurred by comparing specVersion
// at block 1 vs current block.
// Called from any node.
//
// Usage in ZNDSL:
//   leafchain-a-collator-1: js-script ./scripts/zombienet-js/verify-upgrade.js within 60 seconds
//
// Returns the new specVersion on success.
// Follows upstream pattern from cumulus/zombienet/tests/runtime_upgrade.js

const assert = require("assert");

async function run(nodeName, networkInfo, _args) {
  const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
  const api = await zombie.connect(wsUri, userDefinedTypes);

  // specVersion at block 1 (genesis)
  const hashAtBlock1 = await api.rpc.chain.getBlockHash(1);
  const versionAtBlock1 = await api.rpc.state.getRuntimeVersion(
    hashAtBlock1.toHuman()
  );

  // specVersion at current head
  const currentHeader = await api.rpc.chain.getHeader();
  const hashAtCurrent = await api.rpc.chain.getBlockHash(
    currentHeader.number.toHuman()
  );
  const versionAtCurrent = await api.rpc.state.getRuntimeVersion(
    hashAtCurrent.toHuman()
  );

  const oldVersion = parseInt(versionAtBlock1.specVersion.toHuman(), 10);
  const currentVersion = parseInt(
    versionAtCurrent.specVersion.toHuman(),
    10
  );

  console.log(`specVersion at block 1: ${oldVersion}`);
  console.log(`specVersion at current: ${currentVersion}`);

  assert.ok(
    currentVersion > oldVersion,
    `Runtime upgrade not detected: specVersion ${currentVersion} should be > ${oldVersion}`
  );

  console.log(
    `Runtime upgrade verified: ${oldVersion} -> ${currentVersion}`
  );

  return currentVersion;
}

module.exports = { run };
