// Open bidirectional HRMP channels between two parachains via sudo batch.
// Called from relay chain node (alice).
//
// Usage in ZNDSL:
//   alice: js-script ./scripts/zombienet-js/open-hrmp-channels.js with "2000,2001,8,512" within 60 seconds
//
// Args: "senderParaId,recipientParaId,maxCapacity,maxMessageSize"

async function run(nodeName, networkInfo, args) {
  const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
  const api = await zombie.connect(wsUri, userDefinedTypes);

  await zombie.util.cryptoWaitReady();

  const keyring = new zombie.Keyring({ type: "sr25519" });
  const alice = keyring.addFromUri("//Alice");

  // zombienet splits "with" args by comma into an array
  const sender = parseInt(args[0], 10);
  const recipient = parseInt(args[1], 10);
  const maxCapacity = parseInt(args[2], 10);
  const maxMessageSize = parseInt(args[3], 10);
  console.log(
    `Opening bidirectional HRMP channels: ${sender} <-> ${recipient} (capacity=${maxCapacity}, msgSize=${maxMessageSize})`
  );

  const calls = [
    api.tx.parasSudoWrapper.sudoEstablishHrmpChannel(
      sender,
      recipient,
      maxCapacity,
      maxMessageSize
    ),
    api.tx.parasSudoWrapper.sudoEstablishHrmpChannel(
      recipient,
      sender,
      maxCapacity,
      maxMessageSize
    ),
  ];

  const sudoBatch = api.tx.sudo.sudo(api.tx.utility.batch(calls));

  await new Promise(async (resolve, reject) => {
    const unsub = await sudoBatch.signAndSend(alice, (result) => {
      console.log(`Current status is ${result.status}`);
      if (result.status.isInBlock) {
        console.log(
          `Transaction included at blockHash ${result.status.asInBlock}`
        );
      } else if (result.status.isFinalized) {
        console.log(
          `Transaction finalized at blockHash ${result.status.asFinalized}`
        );
        unsub();
        return resolve();
      } else if (result.isError) {
        console.log(`Transaction error`);
        unsub();
        return resolve();
      }
    });
  });

  return 0;
}

module.exports = { run };
