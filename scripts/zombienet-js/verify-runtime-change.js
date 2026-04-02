// Verify that the runtime code has changed (e.g. after a dummy upgrade).
// Unlike verify-upgrade.js which checks specVersion, this checks the
// actual WASM code hash — works for both dummy and real upgrades.
//
// Usage in ZNDSL:
//   leafchain-a-collator-1: js-script ./zombienet-js/verify-runtime-change.js with "16" within 120 seconds
//
// Args: "expectedSpecVersion" (optional — if provided, asserts specVersion matches)
//
// Polls until runtime code hash differs from block 1, or specVersion matches expected.

async function run(nodeName, networkInfo, args) {
  const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
  const api = await zombie.connect(wsUri, userDefinedTypes);

  const expectedSpec = args && args[0] ? parseInt(args[0], 10) : null;

  // Get code hash at block 1
  const hashAtBlock1 = await api.rpc.chain.getBlockHash(1);
  const codeAtBlock1 = await api.rpc.state.getStorage(":code", hashAtBlock1);
  const codeHashBefore = codeAtBlock1.hash.toHex();

  console.log(`Code hash at block 1: ${codeHashBefore.slice(0, 18)}...`);

  // Get code hash at current head
  const currentHeader = await api.rpc.chain.getHeader();
  const codeAtCurrent = await api.rpc.state.getStorage(":code");
  const codeHashAfter = codeAtCurrent.hash.toHex();

  console.log(`Code hash at current (#${currentHeader.number}): ${codeHashAfter.slice(0, 18)}...`);

  if (codeHashBefore !== codeHashAfter) {
    console.log("Runtime code hash CHANGED — upgrade detected");
  } else {
    console.log("Runtime code hash unchanged");
  }

  // Check specVersion if expected value provided
  const currentSpec = (await api.rpc.state.getRuntimeVersion()).specVersion.toNumber();
  console.log(`Current specVersion: ${currentSpec}`);

  if (expectedSpec !== null) {
    if (currentSpec !== expectedSpec) {
      throw new Error(`specVersion mismatch: got ${currentSpec}, expected ${expectedSpec}`);
    }
    console.log(`specVersion matches expected: ${expectedSpec}`);
  }

  // At least one of: code hash changed OR specVersion different from genesis
  const genesisSpec = (await api.rpc.state.getRuntimeVersion(hashAtBlock1.toHuman())).specVersion.toNumber();
  const upgraded = codeHashBefore !== codeHashAfter || currentSpec !== genesisSpec;

  if (!upgraded) {
    throw new Error("No runtime change detected: code hash and specVersion both unchanged");
  }

  console.log("Runtime change verification PASSED");
  return currentSpec;
}

module.exports = { run };
