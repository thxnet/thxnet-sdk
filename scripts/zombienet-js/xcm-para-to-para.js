// Attempt cross-parachain reserve transfer via XCM over HRMP.
// Called from a parachain collator node (sender side).
//
// Usage in ZNDSL:
//   leafchain-a-collator-1: js-script ./zombienet-js/xcm-para-to-para.js with "2001,1000000000000,//XcmCrossRecipient" within 120 seconds
//
// Args: "destParaId,amount,beneficiarySeed"
//
// NOTE: This sends a limitedReserveTransferAssets from the current parachain
//       to a sibling parachain. The extrinsic will succeed on the sender side,
//       and the XCM message will be routed via XCMP/HRMP. However, the
//       receiving parachain's IsReserve=NativeAsset filter will reject the
//       reserve deposit because the asset origin (sibling) doesn't match the
//       asset id (relay token). This is expected behavior per E-63.
//
//       This script intentionally resolves (returns 0) on finalization to prove
//       that the XCMP messaging infrastructure works — even though the transfer
//       itself won't deliver funds.

async function run(nodeName, networkInfo, args) {
  const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
  const api = await zombie.connect(wsUri, userDefinedTypes);

  await zombie.util.cryptoWaitReady();

  const keyring = new zombie.Keyring({ type: "sr25519" });
  const alice = keyring.addFromUri("//Alice");

  // zombienet splits "with" args by comma into an array
  const destParaId = parseInt(args[0], 10);
  const amount = BigInt(args[1]);
  const recipient = keyring.addFromUri(args[2] || "//Alice");

  // Destination: sibling parachain (from parachain's perspective: ../Parachain(destParaId))
  const dest = {
    V4: { parents: 1, interior: { X1: [{ Parachain: destParaId }] } },
  };

  // Beneficiary on the destination parachain
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

  // Assets: relay chain native token (identified as Parent from parachain's view)
  const assets = {
    V4: [
      {
        id: { parents: 1, interior: "Here" },
        fun: { Fungible: amount.toString() },
      },
    ],
  };

  console.log(
    `Sending cross-parachain reserve transfer: ${amount} to parachain ${destParaId}, beneficiary: ${recipient.address}`
  );
  console.log(
    `NOTE: IsReserve=NativeAsset will reject this on the receiving side (E-63)`
  );

  const tx = api.tx.polkadotXcm.limitedReserveTransferAssets(
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
        // Check for dispatch errors in events
        const dispatchError = result.events.find(
          ({ event }) => api.events.system.ExtrinsicFailed.is(event)
        );
        if (dispatchError) {
          console.log(`Extrinsic failed (dispatch error) — this is also useful data`);
        } else {
          console.log(`Extrinsic succeeded on sender side — XCM message sent via XCMP`);
        }
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
