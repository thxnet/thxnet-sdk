// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Polkadot chain configurations.

use polkadot_primitives::{AccountId, AccountPublic, AssignmentId, ValidatorId};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;

#[cfg(feature = "rococo-native")]
use rococo_runtime as rococo;
use sc_chain_spec::ChainSpecExtension;
#[cfg(any(feature = "westend-native", feature = "rococo-native", feature = "thxnet-native"))]
use sc_chain_spec::ChainType;
#[cfg(any(feature = "westend-native", feature = "rococo-native"))]
use sc_telemetry::TelemetryEndpoints;
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::IdentifyAccount;
#[cfg(feature = "thxnet-native")]
use thxnet_runtime as thxnet;
#[cfg(feature = "thxnet-native")]
use thxnet_runtime_constants::currency::UNITS as THX;
#[cfg(feature = "westend-native")]
use westend_runtime as westend;

#[cfg(feature = "westend-native")]
const WESTEND_STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
#[cfg(feature = "rococo-native")]
const ROCOCO_STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
#[cfg(feature = "rococo-native")]
const VERSI_STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
#[cfg(any(feature = "westend-native", feature = "rococo-native"))]
const DEFAULT_PROTOCOL_ID: &str = "dot";

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: sc_client_api::ForkBlocks<polkadot_primitives::Block>,
	/// Known bad block hashes.
	pub bad_blocks: sc_client_api::BadBlocks<polkadot_primitives::Block>,
	/// The light sync state.
	///
	/// This value will be set by the `sync-state rpc` implementation.
	pub light_sync_state: sc_sync_state_rpc::LightSyncStateExtension,
}

// Generic chain spec, in case when we don't have the native runtime.
pub type GenericChainSpec = sc_service::GenericChainSpec<Extensions>;

/// The `ChainSpec` parameterized for the westend runtime.
#[cfg(feature = "westend-native")]
pub type WestendChainSpec = sc_service::GenericChainSpec<Extensions>;

/// The `ChainSpec` parameterized for the westend runtime.
// Dummy chain spec, but that is fine when we don't have the native runtime.
#[cfg(not(feature = "westend-native"))]
pub type WestendChainSpec = GenericChainSpec;

/// The `ChainSpec` parameterized for the rococo runtime.
#[cfg(feature = "rococo-native")]
pub type RococoChainSpec = sc_service::GenericChainSpec<Extensions>;

/// The `ChainSpec` parameterized for the rococo runtime.
// Dummy chain spec, but that is fine when we don't have the native runtime.
#[cfg(not(feature = "rococo-native"))]
pub type RococoChainSpec = GenericChainSpec;

/// The `ChainSpec` parameterized for the thxnet runtime.
#[cfg(feature = "thxnet-native")]
pub type ThxnetChainSpec = sc_service::GenericChainSpec<Extensions>;

/// The `ChainSpec` parameterized for the thxnet runtime.
// Dummy chain spec, but that is fine when we don't have the native runtime.
#[cfg(not(feature = "thxnet-native"))]
pub type ThxnetChainSpec = GenericChainSpec;

pub fn polkadot_config() -> Result<GenericChainSpec, String> {
	GenericChainSpec::from_json_bytes(&include_bytes!("../chain-specs/polkadot.json")[..])
}

pub fn kusama_config() -> Result<GenericChainSpec, String> {
	GenericChainSpec::from_json_bytes(&include_bytes!("../chain-specs/kusama.json")[..])
}

pub fn westend_config() -> Result<WestendChainSpec, String> {
	WestendChainSpec::from_json_bytes(&include_bytes!("../chain-specs/westend.json")[..])
}

pub fn paseo_config() -> Result<GenericChainSpec, String> {
	GenericChainSpec::from_json_bytes(&include_bytes!("../chain-specs/paseo.json")[..])
}

pub fn rococo_config() -> Result<RococoChainSpec, String> {
	RococoChainSpec::from_json_bytes(&include_bytes!("../chain-specs/rococo.json")[..])
}

/// Westend staging testnet config.
#[cfg(feature = "westend-native")]
pub fn westend_staging_testnet_config() -> Result<WestendChainSpec, String> {
	Ok(WestendChainSpec::builder(
		westend::WASM_BINARY.ok_or("Westend development wasm not available")?,
		Default::default(),
	)
	.with_name("Westend Staging Testnet")
	.with_id("westend_staging_testnet")
	.with_chain_type(ChainType::Live)
	.with_genesis_config_preset_name("staging_testnet")
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(WESTEND_STAGING_TELEMETRY_URL.to_string(), 0)])
			.expect("Westend Staging telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.build())
}

/// Rococo staging testnet config.
#[cfg(feature = "rococo-native")]
pub fn rococo_staging_testnet_config() -> Result<RococoChainSpec, String> {
	Ok(RococoChainSpec::builder(
		rococo::WASM_BINARY.ok_or("Rococo development wasm not available")?,
		Default::default(),
	)
	.with_name("Rococo Staging Testnet")
	.with_id("rococo_staging_testnet")
	.with_chain_type(ChainType::Live)
	.with_genesis_config_preset_name("staging_testnet")
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(ROCOCO_STAGING_TELEMETRY_URL.to_string(), 0)])
			.expect("Rococo Staging telemetry url is valid; qed"),
	)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.build())
}

pub fn versi_chain_spec_properties() -> serde_json::map::Map<String, serde_json::Value> {
	serde_json::json!({
		"ss58Format": 42,
		"tokenDecimals": 12,
		"tokenSymbol": "VRS",
	})
	.as_object()
	.expect("Map given; qed")
	.clone()
}

/// Versi staging testnet config.
#[cfg(feature = "rococo-native")]
pub fn versi_staging_testnet_config() -> Result<RococoChainSpec, String> {
	Ok(RococoChainSpec::builder(
		rococo::WASM_BINARY.ok_or("Versi development wasm not available")?,
		Default::default(),
	)
	.with_name("Versi Staging Testnet")
	.with_id("versi_staging_testnet")
	.with_chain_type(ChainType::Live)
	.with_genesis_config_preset_name("staging_testnet")
	.with_telemetry_endpoints(
		TelemetryEndpoints::new(vec![(VERSI_STAGING_TELEMETRY_URL.to_string(), 0)])
			.expect("Versi Staging telemetry url is valid; qed"),
	)
	.with_protocol_id("versi")
	.with_properties(versi_chain_spec_properties())
	.build())
}

/// Westend development config (single validator Alice)
#[cfg(feature = "westend-native")]
pub fn westend_development_config() -> Result<WestendChainSpec, String> {
	Ok(WestendChainSpec::builder(
		westend::WASM_BINARY.ok_or("Westend development wasm not available")?,
		Default::default(),
	)
	.with_name("Development")
	.with_id("westend_dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.build())
}

/// Rococo development config (single validator Alice)
#[cfg(feature = "rococo-native")]
pub fn rococo_development_config() -> Result<RococoChainSpec, String> {
	Ok(RococoChainSpec::builder(
		rococo::WASM_BINARY.ok_or("Rococo development wasm not available")?,
		Default::default(),
	)
	.with_name("Development")
	.with_id("rococo_dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.build())
}

/// `Versi` development config (single validator Alice)
#[cfg(feature = "rococo-native")]
pub fn versi_development_config() -> Result<RococoChainSpec, String> {
	Ok(RococoChainSpec::builder(
		rococo::WASM_BINARY.ok_or("Versi development wasm not available")?,
		Default::default(),
	)
	.with_name("Development")
	.with_id("versi_dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
	.with_protocol_id("versi")
	.build())
}

/// Westend local testnet config (multivalidator Alice + Bob)
#[cfg(feature = "westend-native")]
pub fn westend_local_testnet_config() -> Result<WestendChainSpec, String> {
	Ok(WestendChainSpec::builder(
		westend::fast_runtime_binary::WASM_BINARY
			.ok_or("Westend development wasm not available")?,
		Default::default(),
	)
	.with_name("Westend Local Testnet")
	.with_id("westend_local_testnet")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_preset_name(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.build())
}

/// Rococo local testnet config (multivalidator Alice + Bob)
#[cfg(feature = "rococo-native")]
pub fn rococo_local_testnet_config() -> Result<RococoChainSpec, String> {
	Ok(RococoChainSpec::builder(
		rococo::fast_runtime_binary::WASM_BINARY.ok_or("Rococo development wasm not available")?,
		Default::default(),
	)
	.with_name("Rococo Local Testnet")
	.with_id("rococo_local_testnet")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_preset_name(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET)
	.with_protocol_id(DEFAULT_PROTOCOL_ID)
	.build())
}

/// `Versi` local testnet config (multivalidator Alice + Bob + Charlie + Dave)
#[cfg(feature = "rococo-native")]
pub fn versi_local_testnet_config() -> Result<RococoChainSpec, String> {
	Ok(RococoChainSpec::builder(
		rococo::WASM_BINARY.ok_or("Rococo development wasm (used for versi) not available")?,
		Default::default(),
	)
	.with_name("Versi Local Testnet")
	.with_id("versi_local_testnet")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_preset_name("versi_local_testnet")
	.with_protocol_id("versi")
	.build())
}

// ---- THXNet helpers ----

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to generate stash, controller and session key from seed (no beefy)
pub fn get_authority_keys_from_seed_no_beefy(
	seed: &str,
) -> (AccountId, AccountId, BabeId, GrandpaId, ValidatorId, AssignmentId, AuthorityDiscoveryId) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<BabeId>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<ValidatorId>(seed),
		get_from_seed::<AssignmentId>(seed),
		get_from_seed::<AuthorityDiscoveryId>(seed),
	)
}

#[cfg(feature = "thxnet-native")]
fn testnet_accounts() -> Vec<AccountId> {
	vec![
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		get_account_id_from_seed::<sr25519::Public>("Bob"),
		get_account_id_from_seed::<sr25519::Public>("Charlie"),
		get_account_id_from_seed::<sr25519::Public>("Dave"),
		get_account_id_from_seed::<sr25519::Public>("Eve"),
		get_account_id_from_seed::<sr25519::Public>("Ferdie"),
		get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
		get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
		get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
		get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
		get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
		get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
	]
}

/// The default parachains host configuration (used by thxnet genesis).
#[cfg(feature = "thxnet-native")]
fn default_parachains_host_configuration(
) -> polkadot_runtime_parachains::configuration::HostConfiguration<polkadot_primitives::BlockNumber>
{
	use polkadot_primitives::{
		node_features::FeatureIndex, ApprovalVotingParams, AsyncBackingParams, SchedulerParams,
		MAX_CODE_SIZE, MAX_POV_SIZE,
	};

	polkadot_runtime_parachains::configuration::HostConfiguration {
		validation_upgrade_cooldown: 2u32,
		validation_upgrade_delay: 2,
		code_retention_period: 1200,
		max_code_size: MAX_CODE_SIZE,
		max_pov_size: MAX_POV_SIZE,
		max_head_data_size: 32 * 1024,
		max_upward_queue_count: 8,
		max_upward_queue_size: 1024 * 1024,
		max_downward_message_size: 1024 * 1024,
		max_upward_message_size: 50 * 1024,
		max_upward_message_num_per_candidate: 5,
		hrmp_sender_deposit: 0,
		hrmp_recipient_deposit: 0,
		hrmp_channel_max_capacity: 8,
		hrmp_channel_max_total_size: 8 * 1024,
		hrmp_max_parachain_inbound_channels: 4,
		hrmp_channel_max_message_size: 1024 * 1024,
		hrmp_max_parachain_outbound_channels: 4,
		hrmp_max_message_num_per_candidate: 5,
		dispute_period: 6,
		no_show_slots: 2,
		n_delay_tranches: 25,
		needed_approvals: 2,
		relay_vrf_modulo_samples: 2,
		zeroth_delay_tranche_width: 0,
		minimum_validation_upgrade_delay: 5,
		async_backing_params: AsyncBackingParams {
			max_candidate_depth: 3,
			allowed_ancestry_len: 2,
		},
		node_features: bitvec::vec::BitVec::from_element(
			1u8 << (FeatureIndex::ElasticScalingMVP as usize) |
				1u8 << (FeatureIndex::EnableAssignmentsV2 as usize) |
				1u8 << (FeatureIndex::CandidateReceiptV2 as usize),
		),
		scheduler_params: SchedulerParams {
			lookahead: 2,
			group_rotation_frequency: 20,
			paras_availability_period: 4,
			..Default::default()
		},
		approval_voting_params: ApprovalVotingParams { max_approval_coalesce_count: 5 },
		..Default::default()
	}
}

#[cfg(feature = "thxnet-native")]
fn thxnet_session_keys(
	babe: BabeId,
	grandpa: GrandpaId,
	para_validator: ValidatorId,
	para_assignment: AssignmentId,
	authority_discovery: AuthorityDiscoveryId,
) -> thxnet::SessionKeys {
	thxnet::SessionKeys { babe, grandpa, para_validator, para_assignment, authority_discovery }
}

/// Returns the properties for the [`ThxnetChainSpec`].
#[cfg(feature = "thxnet-native")]
pub fn thxnet_chain_spec_properties() -> serde_json::map::Map<String, serde_json::Value> {
	serde_json::json!({
		"tokenSymbol": "THX",
		"tokenDecimals": 10,
		"ss58Format": 42,
	})
	.as_object()
	.expect("Map given; qed")
	.clone()
}

/// Helper function to create thxnet `RuntimeGenesisConfig` for testing
#[cfg(feature = "thxnet-native")]
pub fn thxnet_testnet_genesis(
	initial_authorities: Vec<(
		AccountId,
		AccountId,
		BabeId,
		GrandpaId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
	)>,
	root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
) -> serde_json::Value {
	let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(testnet_accounts);

	const ENDOWMENT: u128 = 1_000_000 * THX;
	const STASH: u128 = 100 * THX;

	serde_json::json!({
		"balances": {
			"balances": endowed_accounts.iter().map(|k| (k.clone(), ENDOWMENT)).collect::<Vec<_>>(),
		},
		"session": {
			"keys": initial_authorities
				.iter()
				.map(|x| {
					(
						x.0.clone(),
						x.0.clone(),
						thxnet_session_keys(
							x.2.clone(),
							x.3.clone(),
							x.4.clone(),
							x.5.clone(),
							x.6.clone(),
						),
					)
				})
				.collect::<Vec<_>>(),
		},
		"staking": {
			"minimumValidatorCount": 1,
			"validatorCount": initial_authorities.len() as u32,
			"stakers": initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.0.clone(), STASH, thxnet::StakerStatus::<AccountId>::Validator))
				.collect::<Vec<_>>(),
			"invulnerables": initial_authorities.iter().map(|x| x.0.clone()).collect::<Vec<_>>(),
			"forceEra": "NotForcing",
			"slashRewardFraction": sp_runtime::Perbill::from_percent(10),
		},
		"babe": {
			"epochConfig": Some(thxnet::BABE_GENESIS_EPOCH_CONFIG),
		},
		"sudo": { "key": Some(root_key) },
		"configuration": {
			"config": default_parachains_host_configuration(),
		},
	})
}

#[cfg(feature = "thxnet-native")]
fn thxnet_development_config_genesis() -> serde_json::Value {
	thxnet_testnet_genesis(
		vec![get_authority_keys_from_seed_no_beefy("Alice")],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// THXnet development config (single validator Alice)
#[cfg(feature = "thxnet-native")]
pub fn thxnet_development_config() -> Result<ThxnetChainSpec, String> {
	Ok(ThxnetChainSpec::builder(
		thxnet::WASM_BINARY.ok_or("THXnet development wasm not available")?,
		Default::default(),
	)
	.with_name("THXnet Development")
	.with_id("thxnet_dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(thxnet_development_config_genesis())
	.with_protocol_id("thxnet")
	.with_properties(thxnet_chain_spec_properties())
	.build())
}

#[cfg(feature = "thxnet-native")]
fn thxnet_local_testnet_genesis() -> serde_json::Value {
	thxnet_testnet_genesis(
		vec![
			get_authority_keys_from_seed_no_beefy("Alice"),
			get_authority_keys_from_seed_no_beefy("Bob"),
		],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
	)
}

/// THXnet local testnet config (multivalidator Alice + Bob)
#[cfg(feature = "thxnet-native")]
pub fn thxnet_local_testnet_config() -> Result<ThxnetChainSpec, String> {
	Ok(ThxnetChainSpec::builder(
		thxnet::WASM_BINARY.ok_or("THXnet development wasm not available")?,
		Default::default(),
	)
	.with_name("THXnet Local Testnet")
	.with_id("thxnet_local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(thxnet_local_testnet_genesis())
	.with_protocol_id("thxnet")
	.with_properties(thxnet_chain_spec_properties())
	.build())
}
