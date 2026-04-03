// Verify that an HRMP egress channel to a sibling parachain is open.
// Called from a parachain collator node.
//
// Usage in ZNDSL:
//   leafchain-a-collator-1: js-script ./zombienet-js/verify-hrmp-channel.js with "2001" within 120 seconds
//
// Args: "siblingParaId"
//
// Polls parachainSystem.relevantMessagingState() until egressChannels
// contains the target sibling. Retries every 6s (one relay block).

async function run(nodeName, networkInfo, args) {
  const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
  const api = await zombie.connect(wsUri, userDefinedTypes);

  const sibling = parseInt(args[0], 10);
  console.log(`Waiting for HRMP egress channel to parachain ${sibling}...`);

  let attempts = 0;
  const maxAttempts = 40; // 40 * 6s = 240s max

  while (attempts < maxAttempts) {
    const messagingStateOpt =
      await api.query.parachainSystem.relevantMessagingState();
    const messagingState = api.createType(
      "Option<CumulusPalletParachainSystemRelayStateSnapshotMessagingStateSnapshot>",
      messagingStateOpt
    );

    if (messagingState.isSome) {
      const egressChannels = messagingState.unwrap().egressChannels;
      const found = egressChannels.find((x) => x[0] == sibling);
      if (found) {
        console.log(
          `HRMP egress channel to ${sibling} is OPEN (after ${attempts + 1} attempts)`
        );
        return 0;
      }
    }

    attempts++;
    await new Promise((resolve) => setTimeout(resolve, 6000));
  }

  throw new Error(
    `HRMP egress channel to ${sibling} not found after ${maxAttempts} attempts`
  );
}

module.exports = { run };
