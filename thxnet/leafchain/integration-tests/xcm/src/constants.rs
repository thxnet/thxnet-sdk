//! Constants and genesis configurations for XCM integration tests
//!
//! NOTE: The relay chain (THXnet) requires a complex genesis setup with validators,
//! session keys, and staking. For integration testing, consider using a simpler
//! test runtime or mocking the relay chain behavior.

use sp_core::storage::Storage;
use sp_runtime::{BuildStorage, Perbill};
use xcm_emulator::{get_account_id_from_seed, AccountId};
use sp_core::{sr25519, Pair, Public};

// Authority key types
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use polkadot_primitives::{ValidatorId, AssignmentId};

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
        im_online: ImOnlineId,
        para_validator: ValidatorId,
        para_assignment: AssignmentId,
        authority_discovery: AuthorityDiscoveryId,
    ) -> thxnet_runtime::SessionKeys {
        thxnet_runtime::SessionKeys {
            grandpa,
            babe,
            im_online,
            para_validator,
            para_assignment,
            authority_discovery,
        }
    }

    /// Get initial authorities (validators) for genesis
    /// Returns (stash, controller, session_keys_tuple...)
    fn initial_authorities() -> Vec<(
        AccountId,  // stash
        AccountId,  // controller (same as stash for simplicity)
        GrandpaId,
        BabeId,
        ImOnlineId,
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
                get_from_seed::<ImOnlineId>("Alice"),
                get_from_seed::<ValidatorId>("Alice"),
                get_from_seed::<AssignmentId>("Alice"),
                get_from_seed::<AuthorityDiscoveryId>("Alice"),
            ),
            (
                get_account_id_from_seed::<sr25519::Public>("Bob"),
                get_account_id_from_seed::<sr25519::Public>("Bob"),
                get_from_seed::<GrandpaId>("Bob"),
                get_from_seed::<BabeId>("Bob"),
                get_from_seed::<ImOnlineId>("Bob"),
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
            system: thxnet_runtime::SystemConfig {
                code: thxnet_runtime::WASM_BINARY
                    .expect("WASM binary not available for thxnet runtime")
                    .to_vec(),
                _config: Default::default(),
            },
            balances: thxnet_runtime::BalancesConfig {
                balances: vec![
                    (get_account_id_from_seed::<sr25519::Public>(ALICE), INITIAL_BALANCE),
                    (get_account_id_from_seed::<sr25519::Public>(BOB), INITIAL_BALANCE),
                    (get_account_id_from_seed::<sr25519::Public>(CHARLIE), INITIAL_BALANCE),
                ],
            },
            session: thxnet_runtime::SessionConfig {
                keys: initial_authorities
                    .iter()
                    .map(|x| {
                        (
                            x.0.clone(),  // account id (validator)
                            x.0.clone(),  // validator id (same as account)
                            session_keys(
                                x.2.clone(),  // grandpa
                                x.3.clone(),  // babe
                                x.4.clone(),  // im_online
                                x.5.clone(),  // para_validator
                                x.6.clone(),  // para_assignment
                                x.7.clone(),  // authority_discovery
                            ),
                        )
                    })
                    .collect::<Vec<_>>(),
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
                epoch_config: Some(thxnet_runtime::BABE_GENESIS_EPOCH_CONFIG),
                _config: Default::default(),
            },
            grandpa: Default::default(),
            authority_discovery: Default::default(),
            im_online: Default::default(),
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
        let invulnerables: Vec<(AccountId, general_runtime::AuraId)> = vec![
            (
                get_account_id_from_seed::<sr25519::Public>(ALICE),
                get_from_seed::<general_runtime::AuraId>(ALICE),
            ),
        ];

        let genesis_config = general_runtime::RuntimeGenesisConfig {
            system: general_runtime::SystemConfig {
                code: general_runtime::WASM_BINARY
                    .expect("WASM binary not available for general runtime")
                    .to_vec(),
                _config: Default::default(),
            },
            balances: general_runtime::BalancesConfig {
                balances: vec![
                    (get_account_id_from_seed::<sr25519::Public>(ALICE), INITIAL_BALANCE),
                    (get_account_id_from_seed::<sr25519::Public>(BOB), INITIAL_BALANCE),
                    (get_account_id_from_seed::<sr25519::Public>(CHARLIE), INITIAL_BALANCE),
                ],
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
                    .map(|(acc, aura)| {
                        (
                            acc.clone(),
                            acc,
                            general_runtime::SessionKeys { aura },
                        )
                    })
                    .collect(),
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
        let invulnerables: Vec<(AccountId, general_runtime::AuraId)> = vec![
            (
                get_account_id_from_seed::<sr25519::Public>(BOB),
                get_from_seed::<general_runtime::AuraId>(BOB),
            ),
        ];

        let genesis_config = general_runtime::RuntimeGenesisConfig {
            system: general_runtime::SystemConfig {
                code: general_runtime::WASM_BINARY
                    .expect("WASM binary not available for general runtime")
                    .to_vec(),
                _config: Default::default(),
            },
            balances: general_runtime::BalancesConfig {
                balances: vec![
                    (get_account_id_from_seed::<sr25519::Public>(ALICE), INITIAL_BALANCE),
                    (get_account_id_from_seed::<sr25519::Public>(BOB), INITIAL_BALANCE),
                    (get_account_id_from_seed::<sr25519::Public>(CHARLIE), INITIAL_BALANCE),
                ],
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
                    .map(|(acc, aura)| {
                        (
                            acc.clone(),
                            acc,
                            general_runtime::SessionKeys { aura },
                        )
                    })
                    .collect(),
            },
            aura: Default::default(),
            aura_ext: Default::default(),
            ..Default::default()
        };
        genesis_config.build_storage().unwrap()
    }
}
