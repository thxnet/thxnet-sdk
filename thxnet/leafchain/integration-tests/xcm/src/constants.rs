//! Constants and genesis configurations for XCM integration tests
//!
//! NOTE: The relay chain (THXnet) requires a complex genesis setup with validators,
//! session keys, and staking. For integration testing, consider using a simpler
//! test runtime or mocking the relay chain behavior.

use polkadot_primitives::{MAX_CODE_SIZE, MAX_POV_SIZE};
use polkadot_runtime_parachains::paras::{ParaGenesisArgs, ParaKind};
use sp_core::{sr25519, storage::Storage, Pair, Public};
use sp_runtime::{BuildStorage, Perbill};
use xcm_emulator::{get_account_id_from_seed, AccountId};

// Authority key types
use polkadot_primitives::{AssignmentId, ValidatorId};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;

pub const ALICE: &str = "Alice";
pub const BOB: &str = "Bob";
pub const CHARLIE: &str = "Charlie";

/// Initial balance for test accounts
pub const INITIAL_BALANCE: u128 = 1_000_000_000_000_000; // 1_000_000 tokens (with 9 decimals)

/// Helper function to generate a crypto pair from seed
fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// THXnet Relay Chain genesis configuration
pub mod thxnet {
	use super::*;
	use pallet_staking::Forcing;

	/// Helper to generate session keys for thxnet
	fn session_keys(
		grandpa: GrandpaId,
		babe: BabeId,
		para_validator: ValidatorId,
		para_assignment: AssignmentId,
		authority_discovery: AuthorityDiscoveryId,
	) -> thxnet_runtime::SessionKeys {
		thxnet_runtime::SessionKeys {
			grandpa,
			babe,
			para_validator,
			para_assignment,
			authority_discovery,
		}
	}

	/// Get initial authorities (validators) for genesis
	/// Returns (stash, controller, session_keys_tuple...)
	fn initial_authorities() -> Vec<(
		AccountId, // stash
		AccountId, // controller (same as stash for simplicity)
		GrandpaId,
		BabeId,
		ValidatorId,
		AssignmentId,
		AuthorityDiscoveryId,
	)> {
		vec![
			(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				get_from_seed::<GrandpaId>("Alice"),
				get_from_seed::<BabeId>("Alice"),
				get_from_seed::<ValidatorId>("Alice"),
				get_from_seed::<AssignmentId>("Alice"),
				get_from_seed::<AuthorityDiscoveryId>("Alice"),
			),
			(
				get_account_id_from_seed::<sr25519::Public>("Bob"),
				get_account_id_from_seed::<sr25519::Public>("Bob"),
				get_from_seed::<GrandpaId>("Bob"),
				get_from_seed::<BabeId>("Bob"),
				get_from_seed::<ValidatorId>("Bob"),
				get_from_seed::<AssignmentId>("Bob"),
				get_from_seed::<AuthorityDiscoveryId>("Bob"),
			),
		]
	}

	pub fn genesis() -> Storage {
		let initial_authorities = initial_authorities();
		let stash = INITIAL_BALANCE / 10;

		let genesis_config = thxnet_runtime::RuntimeGenesisConfig {
			system: thxnet_runtime::SystemConfig { _config: Default::default() },
			balances: thxnet_runtime::BalancesConfig {
				balances: vec![
					(get_account_id_from_seed::<sr25519::Public>(ALICE), INITIAL_BALANCE),
					(get_account_id_from_seed::<sr25519::Public>(BOB), INITIAL_BALANCE),
					(get_account_id_from_seed::<sr25519::Public>(CHARLIE), INITIAL_BALANCE),
				],
				dev_accounts: None,
			},
			session: thxnet_runtime::SessionConfig {
				keys: initial_authorities
					.iter()
					.map(|x| {
						(
							x.0.clone(), // account id (validator)
							x.0.clone(), // validator id (same as account)
							session_keys(
								x.2.clone(), // grandpa
								x.3.clone(), // babe
								x.4.clone(), // para_validator
								x.5.clone(), // para_assignment
								x.6.clone(), // authority_discovery
							),
						)
					})
					.collect::<Vec<_>>(),
				non_authority_keys: Default::default(),
			},
			staking: thxnet_runtime::StakingConfig {
				validator_count: initial_authorities.len() as u32,
				minimum_validator_count: 1,
				stakers: initial_authorities
					.iter()
					.map(|x| {
						// (stash, controller, stake, status)
						(x.0.clone(), x.1.clone(), stash, thxnet_runtime::StakerStatus::Validator)
					})
					.collect(),
				invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
				force_era: Forcing::NotForcing,
				slash_reward_fraction: Perbill::from_percent(10),
				..Default::default()
			},
			babe: thxnet_runtime::BabeConfig {
				authorities: Default::default(),
				epoch_config: thxnet_runtime::BABE_GENESIS_EPOCH_CONFIG,
				_config: Default::default(),
			},
			grandpa: Default::default(),
			authority_discovery: Default::default(),
			configuration: polkadot_runtime_parachains::configuration::GenesisConfig {
				config: polkadot_runtime_parachains::configuration::HostConfiguration {
					max_code_size: MAX_CODE_SIZE,
					max_pov_size: MAX_POV_SIZE,
					max_head_data_size: 32 * 1024,
					max_upward_queue_count: 8,
					max_upward_queue_size: 1024 * 1024,
					max_downward_message_size: 1024 * 1024,
					max_upward_message_size: 50 * 1024,
					max_upward_message_num_per_candidate: 5,
					hrmp_channel_max_capacity: 8,
					hrmp_channel_max_total_size: 8 * 1024,
					hrmp_max_parachain_inbound_channels: 4,
					hrmp_channel_max_message_size: 1024 * 1024,
					hrmp_max_parachain_outbound_channels: 4,
					hrmp_max_message_num_per_candidate: 5,
					scheduler_params: polkadot_primitives::SchedulerParams {
						group_rotation_frequency: 20,
						paras_availability_period: 4,
						..Default::default()
					},
					no_show_slots: 2,
					n_delay_tranches: 25,
					needed_approvals: 2,
					relay_vrf_modulo_samples: 2,
					dispute_period: 6,
					validation_upgrade_cooldown: 2,
					validation_upgrade_delay: 2,
					code_retention_period: 1200,
					minimum_validation_upgrade_delay: 5,
					..Default::default()
				},
				..Default::default()
			},
			// Register parachains so that paras::Heads contains entries.
			// Without this, dmp::can_queue_downward_message returns Unroutable
			// because it checks paras::Heads::contains_key(para_id).
			paras: polkadot_runtime_parachains::paras::GenesisConfig {
				_config: Default::default(),
				paras: vec![
					(
						2000.into(),
						ParaGenesisArgs {
							genesis_head: polkadot_primitives::HeadData(vec![0u8]),
							validation_code: polkadot_primitives::ValidationCode(vec![0u8]),
							para_kind: ParaKind::Parachain,
						},
					),
					(
						2001.into(),
						ParaGenesisArgs {
							genesis_head: polkadot_primitives::HeadData(vec![0u8]),
							validation_code: polkadot_primitives::ValidationCode(vec![0u8]),
							para_kind: ParaKind::Parachain,
						},
					),
				],
			},
			sudo: thxnet_runtime::SudoConfig {
				key: Some(get_account_id_from_seed::<sr25519::Public>(ALICE)),
			},
			..Default::default()
		};
		genesis_config.build_storage().unwrap()
	}
}

/// LeafchainA (Para ID: 2000) genesis configuration
pub mod leafchain_a {
	use super::*;

	pub const PARA_ID: u32 = 2000;

	pub fn genesis() -> Storage {
		let invulnerables: Vec<(AccountId, general_runtime::AuraId)> = vec![(
			get_account_id_from_seed::<sr25519::Public>(ALICE),
			get_from_seed::<general_runtime::AuraId>(ALICE),
		)];

		let genesis_config = general_runtime::RuntimeGenesisConfig {
			system: general_runtime::SystemConfig { _config: Default::default() },
			balances: general_runtime::BalancesConfig {
				balances: vec![
					(get_account_id_from_seed::<sr25519::Public>(ALICE), INITIAL_BALANCE),
					(get_account_id_from_seed::<sr25519::Public>(BOB), INITIAL_BALANCE),
					(get_account_id_from_seed::<sr25519::Public>(CHARLIE), INITIAL_BALANCE),
				],
				dev_accounts: None,
			},
			parachain_info: general_runtime::ParachainInfoConfig {
				parachain_id: PARA_ID.into(),
				_config: Default::default(),
			},
			collator_selection: general_runtime::CollatorSelectionConfig {
				invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect(),
				candidacy_bond: 0,
				..Default::default()
			},
			session: general_runtime::SessionConfig {
				keys: invulnerables
					.into_iter()
					.map(|(acc, aura)| (acc.clone(), acc, general_runtime::SessionKeys { aura }))
					.collect(),
				non_authority_keys: Default::default(),
			},
			aura: Default::default(),
			aura_ext: Default::default(),
			..Default::default()
		};
		genesis_config.build_storage().unwrap()
	}
}

/// LeafchainB (Para ID: 2001) genesis configuration
pub mod leafchain_b {
	use super::*;

	pub const PARA_ID: u32 = 2001;

	pub fn genesis() -> Storage {
		let invulnerables: Vec<(AccountId, general_runtime::AuraId)> = vec![(
			get_account_id_from_seed::<sr25519::Public>(BOB),
			get_from_seed::<general_runtime::AuraId>(BOB),
		)];

		let genesis_config = general_runtime::RuntimeGenesisConfig {
			system: general_runtime::SystemConfig { _config: Default::default() },
			balances: general_runtime::BalancesConfig {
				balances: vec![
					(get_account_id_from_seed::<sr25519::Public>(ALICE), INITIAL_BALANCE),
					(get_account_id_from_seed::<sr25519::Public>(BOB), INITIAL_BALANCE),
					(get_account_id_from_seed::<sr25519::Public>(CHARLIE), INITIAL_BALANCE),
				],
				dev_accounts: None,
			},
			parachain_info: general_runtime::ParachainInfoConfig {
				parachain_id: PARA_ID.into(),
				_config: Default::default(),
			},
			collator_selection: general_runtime::CollatorSelectionConfig {
				invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect(),
				candidacy_bond: 0,
				..Default::default()
			},
			session: general_runtime::SessionConfig {
				keys: invulnerables
					.into_iter()
					.map(|(acc, aura)| (acc.clone(), acc, general_runtime::SessionKeys { aura }))
					.collect(),
				non_authority_keys: Default::default(),
			},
			aura: Default::default(),
			aura_ext: Default::default(),
			..Default::default()
		};
		genesis_config.build_storage().unwrap()
	}
}
