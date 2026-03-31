// Send DMP reserve transfer from relay chain to a parachain.
// Called from relay chain node (alice).
//
// Usage in ZNDSL:
//   alice: js-script ./scripts/zombienet-js/xcm-relay-to-para.js with "2000,1000000000000,//DmpTestRecipient" within 60 seconds
//
// Args: "destParaId,amount,beneficiarySeed"
//   beneficiarySeed: sr25519 derivation path for the recipient (use a non-endowed seed for meaningful testing)
//
// NOTE: Only DMP (relay→para) works. UMP is blocked by DenyReserveTransferToRelayChain (E-62).
//       XCMP reserve transfers won't transfer balance due to IsReserve=NativeAsset (E-63).

async function run(nodeName, networkInfo, args) {
  const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
  const api = await zombie.connect(wsUri, userDefinedTypes);

  await zombie.util.cryptoWaitReady();

  const keyring = new zombie.Keyring({ type: "sr25519" });
  const alice = keyring.addFromUri("//Alice");

  // zombienet splits "with" args by comma into an array
  const paraId = parseInt(args[0], 10);
  const amount = BigInt(args[1]);
  const recipient = keyring.addFromUri(args[2] || "//Alice");

  // XCM v4 Location for destination parachain
  const dest = { V4: { parents: 0, interior: { X1: [{ Parachain: paraId }] } } };

  // Beneficiary on the parachain
  const beneficiary = {
    V4: {
      parents: 0,
      interior: {
        X1: [
          {
            AccountId32: {
              network: null,
              id: recipient.publicKey,
            },
          },
        ],
      },
    },
  };

  // Assets: native relay token
  const assets = {
    V4: [
      {
        id: { parents: 0, interior: "Here" },
        fun: { Fungible: amount.toString() },
      },
    ],
  };

  console.log(
    `Sending DMP reserve transfer: ${amount} to parachain ${paraId}, beneficiary: ${recipient.address}`
  );

  const tx = api.tx.xcmPallet.limitedReserveTransferAssets(
    dest,
    beneficiary,
    assets,
    0, // fee_asset_item
    "Unlimited"
  );

  await new Promise(async (resolve, reject) => {
    const unsub = await tx.signAndSend(alice, (result) => {
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
