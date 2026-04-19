//! XCM Integration Tests for THXnet Rootchain + Leafchains
//!
//! This module provides xcm-emulator based tests for:
//! - DMP: Downward Message Passing (Relay -> Parachain)
//! - UMP: Upward Message Passing (Parachain -> Relay)
//! - XCMP: Cross-Chain Message Passing (Parachain <-> Parachain)

pub mod constants;

#[cfg(test)]
mod tests;

pub use codec::Encode;
pub use frame_support::assert_ok;
pub use sp_core::{sr25519, Get};
pub use xcm::v5::prelude::*;
pub use xcm_emulator::{
	decl_test_networks, decl_test_parachains, decl_test_relay_chains, get_account_id_from_seed,
	AccountId, Network, ParaId, Parachain, RelayChain, TestExt,
};

// Required for xcm-emulator macros
pub use sp_tracing;
pub use xcm_executor::traits::ConvertLocation;

/// Balance type used in tests
pub type Balance = u128;

// =============================================================================
// Relay Chain: THXnet
// =============================================================================
decl_test_relay_chains! {
	pub struct THXnet {
		genesis = constants::thxnet::genesis(),
		on_init = (),
		runtime = {
			Runtime: thxnet_runtime::Runtime,
			RuntimeOrigin: thxnet_runtime::RuntimeOrigin,
			RuntimeCall: thxnet_runtime::RuntimeCall,
			RuntimeEvent: thxnet_runtime::RuntimeEvent,
			MessageQueue: thxnet_runtime::MessageQueue,
			XcmConfig: thxnet_runtime::xcm_config::XcmConfig,
			SovereignAccountOf: thxnet_runtime::xcm_config::SovereignAccountOf,
			System: thxnet_runtime::System,
			Balances: thxnet_runtime::Balances,
		},
		pallets_extra = {
			XcmPallet: thxnet_runtime::XcmPallet,
		}
	}
}

// =============================================================================
// Parachains: Two Leafchain instances for XCMP testing
// =============================================================================
decl_test_parachains! {
	pub struct LeafchainA {
		genesis = constants::leafchain_a::genesis(),
		on_init = (),
		runtime = {
			Runtime: general_runtime::Runtime,
			RuntimeOrigin: general_runtime::RuntimeOrigin,
			RuntimeCall: general_runtime::RuntimeCall,
			RuntimeEvent: general_runtime::RuntimeEvent,
			XcmpMessageHandler: general_runtime::XcmpQueue,
			DmpMessageHandler: general_runtime::DmpQueue,
			LocationToAccountId: general_runtime::xcm_config::LocationToAccountId,
			System: general_runtime::System,
			Balances: general_runtime::Balances,
			ParachainSystem: general_runtime::ParachainSystem,
			ParachainInfo: general_runtime::ParachainInfo,
		},
		pallets_extra = {
			PolkadotXcm: general_runtime::PolkadotXcm,
		}
	},
	pub struct LeafchainB {
		genesis = constants::leafchain_b::genesis(),
		on_init = (),
		runtime = {
			Runtime: general_runtime::Runtime,
			RuntimeOrigin: general_runtime::RuntimeOrigin,
			RuntimeCall: general_runtime::RuntimeCall,
			RuntimeEvent: general_runtime::RuntimeEvent,
			XcmpMessageHandler: general_runtime::XcmpQueue,
			DmpMessageHandler: general_runtime::DmpQueue,
			LocationToAccountId: general_runtime::xcm_config::LocationToAccountId,
			System: general_runtime::System,
			Balances: general_runtime::Balances,
			ParachainSystem: general_runtime::ParachainSystem,
			ParachainInfo: general_runtime::ParachainInfo,
		},
		pallets_extra = {
			PolkadotXcm: general_runtime::PolkadotXcm,
		}
	}
}

// =============================================================================
// Network: THXnet + 2 Leafchains
// =============================================================================
decl_test_networks! {
	pub struct THXnetNetwork {
		relay_chain = THXnet,
		parachains = vec![
			LeafchainA,
			LeafchainB,
		],
	}
}
