//! Local development chain specifications
//!
//! These chain specs use seed-based test accounts (Alice, Bob, Charlie, etc.)
//! for easy local development and testing without needing actual production keys.

use crate::chain_spec::{
	get_account_id_from_seed, get_collator_keys_from_seed, Extensions, SAFE_XCM_VERSION,
};
use cumulus_primitives_core::ParaId;
use general_runtime::{
	AccountId, AuraId, Balance, BalancesConfig, CollatorSelectionConfig, ParachainInfoConfig,
	PolkadotXcmConfig, RuntimeGenesisConfig, SessionConfig, SessionKeys, SudoConfig, SystemConfig,
	EXISTENTIAL_DEPOSIT, WASM_BINARY,
};
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

/// Generate a genesis config for local development
fn local_genesis(
	root_key: AccountId,
	endowed_accounts: Vec<(AccountId, Balance)>,
	invulnerables: Vec<(AccountId, AuraId)>,
	id: ParaId,
) -> RuntimeGenesisConfig {
	RuntimeGenesisConfig {
		system: SystemConfig {
			code: WASM_BINARY.expect("WASM binary was not built, please build it!").to_vec(),
			_config: Default::default(),
		},
		balances: BalancesConfig { balances: endowed_accounts },
		parachain_info: ParachainInfoConfig { parachain_id: id, _config: Default::default() },
		collator_selection: CollatorSelectionConfig {
			invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect(),
			candidacy_bond: EXISTENTIAL_DEPOSIT * 16,
			..Default::default()
		},
		session: SessionConfig {
			keys: invulnerables
				.into_iter()
				.map(|(acc, aura)| {
					(
						acc.clone(),          // account id
						acc,                  // validator id
						SessionKeys { aura }, // session keys
					)
				})
				.collect(),
		},
		aura: Default::default(),
		aura_ext: Default::default(),
		parachain_system: Default::default(),
		polkadot_xcm: PolkadotXcmConfig {
			safe_xcm_version: Some(SAFE_XCM_VERSION),
			_config: Default::default(),
		},
		transaction_payment: Default::default(),
		assets: Default::default(),
		sudo: SudoConfig { key: Some(root_key) },
	}
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

	crate::chain_spec::ChainSpec::from_genesis(
		// Name
		"Leafchain A Local",
		// ID
		"leafchain_a_local",
		ChainType::Local,
		move || {
			local_genesis(
				root_key.clone(),
				endowed_accounts.clone(),
				invulnerables.clone(),
				para_id.into(),
			)
		},
		// Bootnodes
		Vec::new(),
		// Telemetry
		None,
		// Protocol ID
		Some("leafchain-a-local"),
		// Fork ID
		None,
		// Properties
		Some(make_properties("LOCA", 12, 42)),
		// Extensions
		Extensions { rootchain: ROOTCHAIN_LOCAL_NAME.into(), leafchain_id: para_id },
	)
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

	crate::chain_spec::ChainSpec::from_genesis(
		// Name
		"Leafchain B Local",
		// ID
		"leafchain_b_local",
		ChainType::Local,
		move || {
			local_genesis(
				root_key.clone(),
				endowed_accounts.clone(),
				invulnerables.clone(),
				para_id.into(),
			)
		},
		// Bootnodes
		Vec::new(),
		// Telemetry
		None,
		// Protocol ID
		Some("leafchain-b-local"),
		// Fork ID
		None,
		// Properties
		Some(make_properties("LOCB", 12, 42)),
		// Extensions
		Extensions { rootchain: ROOTCHAIN_LOCAL_NAME.into(), leafchain_id: para_id },
	)
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

	crate::chain_spec::ChainSpec::from_genesis(
		// Name
		"Leafchain Development",
		// ID
		"leafchain_dev",
		ChainType::Development,
		move || {
			local_genesis(
				root_key.clone(),
				endowed_accounts.clone(),
				invulnerables.clone(),
				para_id.into(),
			)
		},
		Vec::new(),
		None,
		Some("leafchain-dev"),
		None,
		Some(make_properties("DEV", 12, 42)),
		Extensions { rootchain: ROOTCHAIN_LOCAL_NAME.into(), leafchain_id: para_id },
	)
}
