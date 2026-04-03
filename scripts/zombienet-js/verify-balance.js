// Verify that an account's free balance is at least a specified minimum.
// Called from any node.
//
// Usage in ZNDSL:
//   leafchain-a-collator-1: js-script ./zombienet-js/verify-balance.js with "//DmpTestRecipient,500000000000" within 120 seconds
//
// Args: "seedOrAddress,minBalance"
//   seedOrAddress: sr25519 seed (e.g. "//Alice") or SS58 address
//
// Polls every 6s until balance >= minBalance (or throws after 40 attempts).

async function run(nodeName, networkInfo, args) {
  const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
  const api = await zombie.connect(wsUri, userDefinedTypes);

  await zombie.util.cryptoWaitReady();

  // zombienet splits "with" args by comma into an array
  const seedOrAddress = args[0];
  const minBalance = BigInt(args[1]);

  // Resolve seed to address if it starts with "//"
  let accountAddress = seedOrAddress;
  if (seedOrAddress.startsWith("//")) {
    const keyring = new zombie.Keyring({ type: "sr25519" });
    const pair = keyring.addFromUri(seedOrAddress);
    accountAddress = pair.address;
  }

  console.log(
    `Waiting for balance of ${accountAddress} (from ${seedOrAddress}) to be >= ${minBalance}...`
  );

  let attempts = 0;
  const maxAttempts = 40;

  while (attempts < maxAttempts) {
    const accountData = await api.query.system.account(accountAddress);
    const free = BigInt(accountData.data.free.toString());

    if (free >= minBalance) {
      console.log(`Balance verified: ${free} >= ${minBalance} (after ${attempts + 1} attempts)`);
      return free.toString();
    }

    attempts++;
    console.log(`Attempt ${attempts}: balance = ${free}, waiting...`);
    await new Promise((resolve) => setTimeout(resolve, 6000));
  }

  throw new Error(
    `Balance of ${accountAddress} did not reach ${minBalance} after ${maxAttempts} attempts`
  );
}

module.exports = { run };
