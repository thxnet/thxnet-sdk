//! Local development chain specifications
//!
//! These chain specs use seed-based test accounts (Alice, Bob, Charlie, etc.)
//! for easy local development and testing without needing actual production keys.

use crate::chain_spec::{
	get_account_id_from_seed, get_collator_keys_from_seed, Extensions, SAFE_XCM_VERSION,
};
use cumulus_primitives_core::ParaId;
use general_runtime::{AccountId, AuraId, Balance, EXISTENTIAL_DEPOSIT, WASM_BINARY};
use sc_chain_spec::Properties;
use sc_service::ChainType;
use sp_core::sr25519;

/// Base unit for token amounts (10^12, 12 decimal places like the relay chain).
pub const UNITS: Balance = 1_000_000_000_000;

/// Rootchain names for local development
const ROOTCHAIN_LOCAL_NAME: &str = "thxnet_local";

/// Helper to create chain properties
fn make_properties(symbol: &str, decimals: u32, ss58_format: u32) -> Properties {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), symbol.into());
	properties.insert("tokenDecimals".into(), decimals.into());
	properties.insert("ss58Format".into(), ss58_format.into());
	properties
}

/// Generate a genesis config patch for local development
fn local_genesis_patch(
	root_key: AccountId,
	endowed_accounts: Vec<(AccountId, Balance)>,
	invulnerables: Vec<(AccountId, AuraId)>,
	id: ParaId,
) -> serde_json::Value {
	serde_json::json!({
		"balances": {
			"balances": endowed_accounts.iter().map(|(a, b)| (a, b)).collect::<Vec<_>>(),
		},
		"parachainInfo": {
			"parachainId": id,
		},
		"collatorSelection": {
			"invulnerables": invulnerables.iter().map(|(acc, _)| acc).collect::<Vec<_>>(),
			"candidacyBond": EXISTENTIAL_DEPOSIT * 16,
		},
		"session": {
			"keys": invulnerables
				.iter()
				.map(|(acc, aura)| {
					(
						acc,                                                    // account id
						acc,                                                    // validator id
						general_runtime::SessionKeys { aura: aura.clone() },    // session keys
					)
				})
				.collect::<Vec<_>>(),
		},
		"polkadotXcm": {
			"safeXcmVersion": Some(SAFE_XCM_VERSION),
		},
		"sudo": {
			"key": Some(root_key),
		},
	})
}

/// LeafchainA local development config (Para ID: 2000)
pub fn leafchain_a_local_config() -> crate::chain_spec::ChainSpec {
	let para_id: u32 = 2000;

	// Root account (sudo)
	let root_key = get_account_id_from_seed::<sr25519::Public>("Alice");

	// Endowed accounts with initial balances
	let endowed_accounts: Vec<(AccountId, Balance)> = vec![
		(get_account_id_from_seed::<sr25519::Public>("Alice"), 1_000_000 * UNITS),
		(get_account_id_from_seed::<sr25519::Public>("Bob"), 1_000_000 * UNITS),
		(get_account_id_from_seed::<sr25519::Public>("Charlie"), 1_000_000 * UNITS),
		(get_account_id_from_seed::<sr25519::Public>("Dave"), 1_000_000 * UNITS),
		(get_account_id_from_seed::<sr25519::Public>("Eve"), 1_000_000 * UNITS),
		(get_account_id_from_seed::<sr25519::Public>("Ferdie"), 1_000_000 * UNITS),
	];

	// Collators
	let invulnerables: Vec<(AccountId, AuraId)> =
		vec![get_collator_keys_from_seed("Alice"), get_collator_keys_from_seed("Bob")];

	let wasm_binary = WASM_BINARY.expect("WASM binary was not built, please build it!");

	crate::chain_spec::ChainSpec::builder(
		wasm_binary,
		Extensions { rootchain: ROOTCHAIN_LOCAL_NAME.into(), leafchain_id: para_id },
	)
	.with_name("Leafchain A Local")
	.with_id("leafchain_a_local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(local_genesis_patch(
		root_key,
		endowed_accounts,
		invulnerables,
		para_id.into(),
	))
	.with_protocol_id("leafchain-a-local")
	.with_properties(make_properties("LOCA", 12, 42))
	.build()
}

/// LeafchainB local development config (Para ID: 2001)
pub fn leafchain_b_local_config() -> crate::chain_spec::ChainSpec {
	let para_id: u32 = 2001;

	// Root account (sudo) - use Charlie for LeafchainB
	let root_key = get_account_id_from_seed::<sr25519::Public>("Charlie");

	// Endowed accounts with initial balances
	let endowed_accounts: Vec<(AccountId, Balance)> = vec![
		(get_account_id_from_seed::<sr25519::Public>("Alice"), 1_000_000 * UNITS),
		(get_account_id_from_seed::<sr25519::Public>("Bob"), 1_000_000 * UNITS),
		(get_account_id_from_seed::<sr25519::Public>("Charlie"), 1_000_000 * UNITS),
		(get_account_id_from_seed::<sr25519::Public>("Dave"), 1_000_000 * UNITS),
		(get_account_id_from_seed::<sr25519::Public>("Eve"), 1_000_000 * UNITS),
		(get_account_id_from_seed::<sr25519::Public>("Ferdie"), 1_000_000 * UNITS),
	];

	// Collators - use Charlie and Dave for LeafchainB
	let invulnerables: Vec<(AccountId, AuraId)> =
		vec![get_collator_keys_from_seed("Charlie"), get_collator_keys_from_seed("Dave")];

	let wasm_binary = WASM_BINARY.expect("WASM binary was not built, please build it!");

	crate::chain_spec::ChainSpec::builder(
		wasm_binary,
		Extensions { rootchain: ROOTCHAIN_LOCAL_NAME.into(), leafchain_id: para_id },
	)
	.with_name("Leafchain B Local")
	.with_id("leafchain_b_local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(local_genesis_patch(
		root_key,
		endowed_accounts,
		invulnerables,
		para_id.into(),
	))
	.with_protocol_id("leafchain-b-local")
	.with_properties(make_properties("LOCB", 12, 42))
	.build()
}

/// Development config for single collator testing (Para ID: 2000)
pub fn development_config() -> crate::chain_spec::ChainSpec {
	let para_id: u32 = 2000;

	let root_key = get_account_id_from_seed::<sr25519::Public>("Alice");

	let endowed_accounts: Vec<(AccountId, Balance)> = vec![
		(get_account_id_from_seed::<sr25519::Public>("Alice"), 10_000_000 * UNITS),
		(get_account_id_from_seed::<sr25519::Public>("Bob"), 10_000_000 * UNITS),
	];

	// Single collator for development
	let invulnerables: Vec<(AccountId, AuraId)> = vec![get_collator_keys_from_seed("Alice")];

	let wasm_binary = WASM_BINARY.expect("WASM binary was not built, please build it!");

	crate::chain_spec::ChainSpec::builder(
		wasm_binary,
		Extensions { rootchain: ROOTCHAIN_LOCAL_NAME.into(), leafchain_id: para_id },
	)
	.with_name("Leafchain Development")
	.with_id("leafchain_dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(local_genesis_patch(
		root_key,
		endowed_accounts,
		invulnerables,
		para_id.into(),
	))
	.with_protocol_id("leafchain-dev")
	.with_properties(make_properties("DEV", 12, 42))
	.build()
}
