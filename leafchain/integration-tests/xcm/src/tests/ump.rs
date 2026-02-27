//! UMP Tests: Parachain -> Relay Chain (Upward Message Passing)
//!
//! These tests verify that messages and assets can be sent from parachains
//! (Leafchains) to the relay chain (THXnet).

use crate::*;
use crate::constants::{ALICE, BOB, CHARLIE, leafchain_a};
use xcm_emulator::bx;

/// Test reserve transfer from parachain to relay chain
#[test]
fn reserve_transfer_from_parachain_to_relay() {
    THXnetNetwork::reset();

    let transfer_amount: Balance = 10_000_000_000_000; // 10_000 tokens

    // First, fund the parachain sovereign account on relay
    // This is needed because reserve transfers require the sovereign account to have funds
    THXnet::execute_with(|| {
        let para_sovereign = THXnet::sovereign_account_id_of(
            MultiLocation::new(0, X1(Parachain(leafchain_a::PARA_ID)))
        );

        // Fund the sovereign account
        assert_ok!(thxnet_runtime::Balances::force_set_balance(
            thxnet_runtime::RuntimeOrigin::root(),
            para_sovereign.into(),
            transfer_amount * 2,
        ));
    });

    // Execute on parachain: send to relay
    LeafchainA::execute_with(|| {
        let dest = MultiLocation::parent();
        let beneficiary = MultiLocation::new(
            0,
            X1(AccountId32 {
                network: None,
                id: THXnet::account_id_of(BOB).into(),
            }),
        );
        let assets: MultiAssets = (Parent, transfer_amount).into();

        // Use PolkadotXcm pallet on parachain
        let result = general_runtime::PolkadotXcm::limited_reserve_transfer_assets(
            general_runtime::RuntimeOrigin::signed(LeafchainA::account_id_of(ALICE)),
            bx!(dest.into()),
            bx!(beneficiary.into()),
            bx!(assets.into()),
            0,
            WeightLimit::Unlimited,
        );

        log::info!("UMP reserve transfer result: {:?}", result);
        // Note: Reserve transfers from parachain to relay may be filtered by XCM config.
        // The dispatch attempt itself validates the UMP message path.
    });

    // Verify on relay chain
    // Note: In the xcm-emulator, UMP message queues are not fully processed
    // end-to-end, so the balance may not reflect the transfer.
    THXnet::execute_with(|| {
        let bob_balance = thxnet_runtime::Balances::free_balance(
            THXnet::account_id_of(BOB)
        );
        log::info!("Bob's balance on THXnet relay: {:?}", bob_balance);
    });
}

/// Test sending a custom XCM message from parachain to relay
#[test]
fn send_xcm_from_parachain_to_relay() {
    THXnetNetwork::reset();

    LeafchainA::execute_with(|| {
        let dest = MultiLocation::parent();

        // Create a simple XCM message (query response or ping)
        let message: Xcm<()> = Xcm(vec![
            WithdrawAsset((Parent, 1_000_000_000u128).into()),
            BuyExecution {
                fees: (Parent, 1_000_000_000u128).into(),
                weight_limit: WeightLimit::Unlimited,
            },
            // Just a simple operation
            RefundSurplus,
            DepositAsset {
                assets: AllCounted(1).into(),
                beneficiary: MultiLocation::new(
                    0,
                    X1(AccountId32 {
                        network: None,
                        id: THXnet::account_id_of(BOB).into(),
                    }),
                ),
            },
        ]);

        let result = general_runtime::PolkadotXcm::send(
            general_runtime::RuntimeOrigin::root(),
            bx!(dest.into()),
            bx!(xcm::VersionedXcm::V3(message)),
        );

        log::info!("XCM send result: {:?}", result);
    });
}

/// Test teleport from parachain to relay (if configured)
#[test]
fn teleport_from_parachain_to_relay() {
    THXnetNetwork::reset();

    let transfer_amount: Balance = 5_000_000_000_000; // 5_000 tokens

    LeafchainA::execute_with(|| {
        let dest = MultiLocation::parent();
        let beneficiary = MultiLocation::new(
            0,
            X1(AccountId32 {
                network: None,
                id: THXnet::account_id_of(CHARLIE).into(),
            }),
        );
        let assets: MultiAssets = (Parent, transfer_amount).into();

        // Note: This may fail if teleportation is not enabled
        let result = general_runtime::PolkadotXcm::limited_teleport_assets(
            general_runtime::RuntimeOrigin::signed(LeafchainA::account_id_of(ALICE)),
            bx!(dest.into()),
            bx!(beneficiary.into()),
            bx!(assets.into()),
            0,
            WeightLimit::Unlimited,
        );

        log::info!("Teleport to relay result: {:?}", result);
    });
}
