// Verify that an account's free balance is below a threshold (or zero).
// Used to confirm that a transfer did NOT deliver funds — e.g., due to
// IsReserve rejection on cross-parachain XCM (E-63).
//
// Usage in ZNDSL:
//   leafchain-b-collator-1: js-script ./zombienet-js/verify-no-balance.js with "//XcmCrossRecipient,1000000000" within 60 seconds
//
// Args: "seedOrAddress,maxBalance"
//   seedOrAddress: sr25519 seed (e.g. "//Alice") or SS58 address
//   maxBalance: the balance must be BELOW this value to pass
//
// Waits a few relay blocks to let any pending XCM settle, then checks once.
// If balance < maxBalance, returns 0 (pass). Otherwise throws.

async function run(nodeName, networkInfo, args) {
  const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
  const api = await zombie.connect(wsUri, userDefinedTypes);

  await zombie.util.cryptoWaitReady();

  const seedOrAddress = args[0];
  const maxBalance = BigInt(args[1]);

  // Resolve seed to address if it starts with "//"
  let accountAddress = seedOrAddress;
  if (seedOrAddress.startsWith("//")) {
    const keyring = new zombie.Keyring({ type: "sr25519" });
    const pair = keyring.addFromUri(seedOrAddress);
    accountAddress = pair.address;
  }

  // Wait a few relay blocks (18s = ~3 relay blocks) to let any pending XCM
  // messages arrive and be processed before we check.
  console.log(
    `Waiting 18s for any pending XCM to settle before checking balance of ${accountAddress}...`
  );
  await new Promise((resolve) => setTimeout(resolve, 18000));

  const accountData = await api.query.system.account(accountAddress);
  const free = BigInt(accountData.data.free.toString());

  if (free < maxBalance) {
    console.log(
      `Balance confirmed below threshold: ${free} < ${maxBalance} — transfer did NOT deliver funds (expected)`
    );
    return 0;
  }

  throw new Error(
    `Unexpected balance: ${free} >= ${maxBalance} — transfer was NOT expected to deliver funds (IsReserve=NativeAsset should block cross-parachain reserve transfers)`
  );
}

module.exports = { run };
