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

//! Migration from V4 to V5 of the `parachains_configuration` pallet.
//!
//! THXNet rootchain is live at StorageVersion v4. Polkadot-sdk removed this migration
//! after v1.0.0 because Polkadot/Kusama had already executed it. We port it here so
//! the upgrade chain v4 → v5 → v6 → v7 → ... is unbroken.
//!
//! Changes v4 → v5:
//!   - Added `async_backing_params` (defaults to zeroes — disabled)
//!   - Added `executor_params` (defaults to empty)
//!   - Removed `dispute_conclusion_by_time_out_period`

use crate::configuration::{self, Config, Pallet, MAX_POV_SIZE};
use frame_support::{
	pallet_prelude::*,
	traits::{Defensive, OnRuntimeUpgrade, StorageVersion},
	weights::{constants::WEIGHT_REF_TIME_PER_MILLIS, Weight},
};
use frame_system::pallet_prelude::BlockNumberFor;
use primitives::{AsyncBackingParams, Balance, ExecutorParams, SessionIndex};
use sp_std::vec::Vec;

#[cfg(feature = "try-runtime")]
use sp_std::prelude::*;

// ---------------------------------------------------------------------------
// V4 host configuration — the struct that is on-chain NOW on THXNet rootchain.
// Copied from polkadot v1.0.0 @ de9e147 with Weight already using 2D representation.
// ---------------------------------------------------------------------------
#[derive(parity_scale_codec::Encode, parity_scale_codec::Decode, Debug, Clone)]
pub struct V4HostConfiguration<BlockNumber> {
	pub max_code_size: u32,
	pub max_head_data_size: u32,
	pub max_upward_queue_count: u32,
	pub max_upward_queue_size: u32,
	pub max_upward_message_size: u32,
	pub max_upward_message_num_per_candidate: u32,
	pub hrmp_max_message_num_per_candidate: u32,
	pub validation_upgrade_cooldown: BlockNumber,
	pub validation_upgrade_delay: BlockNumber,
	pub max_pov_size: u32,
	pub max_downward_message_size: u32,
	pub ump_service_total_weight: Weight,
	pub hrmp_max_parachain_outbound_channels: u32,
	pub hrmp_max_parathread_outbound_channels: u32,
	pub hrmp_sender_deposit: Balance,
	pub hrmp_recipient_deposit: Balance,
	pub hrmp_channel_max_capacity: u32,
	pub hrmp_channel_max_total_size: u32,
	pub hrmp_max_parachain_inbound_channels: u32,
	pub hrmp_max_parathread_inbound_channels: u32,
	pub hrmp_channel_max_message_size: u32,
	pub code_retention_period: BlockNumber,
	pub parathread_cores: u32,
	pub parathread_retries: u32,
	pub group_rotation_frequency: BlockNumber,
	pub chain_availability_period: BlockNumber,
	pub thread_availability_period: BlockNumber,
	pub scheduling_lookahead: u32,
	pub max_validators_per_core: Option<u32>,
	pub max_validators: Option<u32>,
	pub dispute_period: SessionIndex,
	pub dispute_post_conclusion_acceptance_period: BlockNumber,
	pub dispute_conclusion_by_time_out_period: BlockNumber,
	pub no_show_slots: u32,
	pub n_delay_tranches: u32,
	pub zeroth_delay_tranche_width: u32,
	pub needed_approvals: u32,
	pub relay_vrf_modulo_samples: u32,
	pub ump_max_individual_weight: Weight,
	pub pvf_checking_enabled: bool,
	pub pvf_voting_ttl: SessionIndex,
	pub minimum_validation_upgrade_delay: BlockNumber,
}

impl<BlockNumber: Default + From<u32>> Default for V4HostConfiguration<BlockNumber> {
	fn default() -> Self {
		Self {
			group_rotation_frequency: 1u32.into(),
			chain_availability_period: 1u32.into(),
			thread_availability_period: 1u32.into(),
			no_show_slots: 1u32.into(),
			validation_upgrade_cooldown: Default::default(),
			validation_upgrade_delay: Default::default(),
			code_retention_period: Default::default(),
			max_code_size: Default::default(),
			max_pov_size: Default::default(),
			max_head_data_size: Default::default(),
			parathread_cores: Default::default(),
			parathread_retries: Default::default(),
			scheduling_lookahead: Default::default(),
			max_validators_per_core: Default::default(),
			max_validators: None,
			dispute_period: 6,
			dispute_post_conclusion_acceptance_period: 100.into(),
			dispute_conclusion_by_time_out_period: 200.into(),
			n_delay_tranches: Default::default(),
			zeroth_delay_tranche_width: Default::default(),
			needed_approvals: Default::default(),
			relay_vrf_modulo_samples: Default::default(),
			max_upward_queue_count: Default::default(),
			max_upward_queue_size: Default::default(),
			max_downward_message_size: Default::default(),
			ump_service_total_weight: Default::default(),
			max_upward_message_size: Default::default(),
			max_upward_message_num_per_candidate: Default::default(),
			hrmp_sender_deposit: Default::default(),
			hrmp_recipient_deposit: Default::default(),
			hrmp_channel_max_capacity: Default::default(),
			hrmp_channel_max_total_size: Default::default(),
			hrmp_max_parachain_inbound_channels: Default::default(),
			hrmp_max_parathread_inbound_channels: Default::default(),
			hrmp_channel_max_message_size: Default::default(),
			hrmp_max_parachain_outbound_channels: Default::default(),
			hrmp_max_parathread_outbound_channels: Default::default(),
			hrmp_max_message_num_per_candidate: Default::default(),
			ump_max_individual_weight: Weight::from_parts(
				20u64 * WEIGHT_REF_TIME_PER_MILLIS,
				MAX_POV_SIZE as u64,
			),
			pvf_checking_enabled: false,
			pvf_voting_ttl: 2u32.into(),
			minimum_validation_upgrade_delay: 2.into(),
		}
	}
}

// ---------------------------------------------------------------------------
// V5 host configuration — intermediate type between v4 and v6.
// Adds async_backing_params + executor_params, removes dispute_conclusion_by_time_out_period.
// ---------------------------------------------------------------------------
#[derive(parity_scale_codec::Encode, parity_scale_codec::Decode, Debug, Clone)]
pub struct V5HostConfiguration<BlockNumber> {
	pub max_code_size: u32,
	pub max_head_data_size: u32,
	pub max_upward_queue_count: u32,
	pub max_upward_queue_size: u32,
	pub max_upward_message_size: u32,
	pub max_upward_message_num_per_candidate: u32,
	pub hrmp_max_message_num_per_candidate: u32,
	pub validation_upgrade_cooldown: BlockNumber,
	pub validation_upgrade_delay: BlockNumber,
	pub async_backing_params: AsyncBackingParams,
	pub max_pov_size: u32,
	pub max_downward_message_size: u32,
	pub ump_service_total_weight: Weight,
	pub hrmp_max_parachain_outbound_channels: u32,
	pub hrmp_max_parathread_outbound_channels: u32,
	pub hrmp_sender_deposit: Balance,
	pub hrmp_recipient_deposit: Balance,
	pub hrmp_channel_max_capacity: u32,
	pub hrmp_channel_max_total_size: u32,
	pub hrmp_max_parachain_inbound_channels: u32,
	pub hrmp_max_parathread_inbound_channels: u32,
	pub hrmp_channel_max_message_size: u32,
	pub executor_params: ExecutorParams,
	pub code_retention_period: BlockNumber,
	pub parathread_cores: u32,
	pub parathread_retries: u32,
	pub group_rotation_frequency: BlockNumber,
	pub chain_availability_period: BlockNumber,
	pub thread_availability_period: BlockNumber,
	pub scheduling_lookahead: u32,
	pub max_validators_per_core: Option<u32>,
	pub max_validators: Option<u32>,
	pub dispute_period: SessionIndex,
	pub dispute_post_conclusion_acceptance_period: BlockNumber,
	pub no_show_slots: u32,
	pub n_delay_tranches: u32,
	pub zeroth_delay_tranche_width: u32,
	pub needed_approvals: u32,
	pub relay_vrf_modulo_samples: u32,
	pub ump_max_individual_weight: Weight,
	pub pvf_checking_enabled: bool,
	pub pvf_voting_ttl: SessionIndex,
	pub minimum_validation_upgrade_delay: BlockNumber,
}

impl<BlockNumber: Default + From<u32>> Default for V5HostConfiguration<BlockNumber> {
	fn default() -> Self {
		Self {
			async_backing_params: AsyncBackingParams {
				max_candidate_depth: 0,
				allowed_ancestry_len: 0,
			},
			group_rotation_frequency: 1u32.into(),
			chain_availability_period: 1u32.into(),
			thread_availability_period: 1u32.into(),
			no_show_slots: 1u32.into(),
			validation_upgrade_cooldown: Default::default(),
			validation_upgrade_delay: Default::default(),
			code_retention_period: Default::default(),
			max_code_size: Default::default(),
			max_pov_size: Default::default(),
			max_head_data_size: Default::default(),
			parathread_cores: Default::default(),
			parathread_retries: Default::default(),
			scheduling_lookahead: Default::default(),
			max_validators_per_core: Default::default(),
			max_validators: None,
			dispute_period: 6,
			dispute_post_conclusion_acceptance_period: 100.into(),
			n_delay_tranches: Default::default(),
			zeroth_delay_tranche_width: Default::default(),
			needed_approvals: Default::default(),
			relay_vrf_modulo_samples: Default::default(),
			max_upward_queue_count: Default::default(),
			max_upward_queue_size: Default::default(),
			max_downward_message_size: Default::default(),
			ump_service_total_weight: Default::default(),
			max_upward_message_size: Default::default(),
			max_upward_message_num_per_candidate: Default::default(),
			hrmp_sender_deposit: Default::default(),
			hrmp_recipient_deposit: Default::default(),
			hrmp_channel_max_capacity: Default::default(),
			hrmp_channel_max_total_size: Default::default(),
			hrmp_max_parachain_inbound_channels: Default::default(),
			hrmp_max_parathread_inbound_channels: Default::default(),
			hrmp_channel_max_message_size: Default::default(),
			hrmp_max_parachain_outbound_channels: Default::default(),
			hrmp_max_parathread_outbound_channels: Default::default(),
			hrmp_max_message_num_per_candidate: Default::default(),
			ump_max_individual_weight: Weight::from_parts(
				20u64 * WEIGHT_REF_TIME_PER_MILLIS,
				MAX_POV_SIZE as u64,
			),
			pvf_checking_enabled: false,
			pvf_voting_ttl: 2u32.into(),
			minimum_validation_upgrade_delay: 2.into(),
			executor_params: Default::default(),
		}
	}
}

// ---------------------------------------------------------------------------
// Storage aliases for decoding v4 and v5 on-chain state.
// ---------------------------------------------------------------------------
mod v4 {
	use super::*;

	#[frame_support::storage_alias]
	pub(crate) type ActiveConfig<T: Config> =
		StorageValue<Pallet<T>, V4HostConfiguration<BlockNumberFor<T>>, OptionQuery>;

	#[frame_support::storage_alias]
	pub(crate) type PendingConfigs<T: Config> = StorageValue<
		Pallet<T>,
		Vec<(SessionIndex, V4HostConfiguration<BlockNumberFor<T>>)>,
		OptionQuery,
	>;
}

mod v5 {
	use super::*;

	#[frame_support::storage_alias]
	pub(crate) type ActiveConfig<T: Config> =
		StorageValue<Pallet<T>, V5HostConfiguration<BlockNumberFor<T>>, OptionQuery>;

	#[frame_support::storage_alias]
	pub(crate) type PendingConfigs<T: Config> = StorageValue<
		Pallet<T>,
		Vec<(SessionIndex, V5HostConfiguration<BlockNumberFor<T>>)>,
		OptionQuery,
	>;
}

// ---------------------------------------------------------------------------
// Migration: v4 → v5
// ---------------------------------------------------------------------------
pub struct MigrateToV5<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> OnRuntimeUpgrade for MigrateToV5<T> {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		log::trace!(target: crate::configuration::LOG_TARGET, "Running pre_upgrade() for MigrateToV5");
		Ok(Vec::new())
	}

	fn on_runtime_upgrade() -> Weight {
		log::info!(target: configuration::LOG_TARGET, "MigrateToV5 started");
		if StorageVersion::get::<Pallet<T>>() == 4 {
			let weight_consumed = migrate_to_v5::<T>();

			log::info!(target: configuration::LOG_TARGET, "MigrateToV5 executed successfully");
			StorageVersion::new(5).put::<Pallet<T>>();

			weight_consumed
		} else {
			log::warn!(target: configuration::LOG_TARGET, "MigrateToV5 should be removed.");
			T::DbWeight::get().reads(1)
		}
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		log::trace!(target: crate::configuration::LOG_TARGET, "Running post_upgrade() for MigrateToV5");
		ensure!(
			StorageVersion::get::<Pallet<T>>() >= 5,
			"Storage version should be >= 5 after the migration"
		);
		Ok(())
	}
}

fn migrate_to_v5<T: Config>() -> Weight {
	// Unusual formatting is justified:
	// - make it easier to verify that fields assign what they supposed to assign.
	// - this code is transient and will be removed after all migrations are done.
	// - this code is important enough to optimize for legibility sacrificing consistency.
	#[rustfmt::skip]
	let translate =
		|pre: V4HostConfiguration<BlockNumberFor<T>>| ->
		V5HostConfiguration<BlockNumberFor<T>>
	{
		V5HostConfiguration {
max_code_size                            : pre.max_code_size,
max_head_data_size                       : pre.max_head_data_size,
max_upward_queue_count                   : pre.max_upward_queue_count,
max_upward_queue_size                    : pre.max_upward_queue_size,
max_upward_message_size                  : pre.max_upward_message_size,
max_upward_message_num_per_candidate     : pre.max_upward_message_num_per_candidate,
hrmp_max_message_num_per_candidate       : pre.hrmp_max_message_num_per_candidate,
validation_upgrade_cooldown              : pre.validation_upgrade_cooldown,
validation_upgrade_delay                 : pre.validation_upgrade_delay,
max_pov_size                             : pre.max_pov_size,
max_downward_message_size                : pre.max_downward_message_size,
ump_service_total_weight                 : pre.ump_service_total_weight,
hrmp_max_parachain_outbound_channels     : pre.hrmp_max_parachain_outbound_channels,
hrmp_max_parathread_outbound_channels    : pre.hrmp_max_parathread_outbound_channels,
hrmp_sender_deposit                      : pre.hrmp_sender_deposit,
hrmp_recipient_deposit                   : pre.hrmp_recipient_deposit,
hrmp_channel_max_capacity                : pre.hrmp_channel_max_capacity,
hrmp_channel_max_total_size              : pre.hrmp_channel_max_total_size,
hrmp_max_parachain_inbound_channels      : pre.hrmp_max_parachain_inbound_channels,
hrmp_max_parathread_inbound_channels     : pre.hrmp_max_parathread_inbound_channels,
hrmp_channel_max_message_size            : pre.hrmp_channel_max_message_size,
code_retention_period                    : pre.code_retention_period,
parathread_cores                         : pre.parathread_cores,
parathread_retries                       : pre.parathread_retries,
group_rotation_frequency                 : pre.group_rotation_frequency,
chain_availability_period                : pre.chain_availability_period,
thread_availability_period               : pre.thread_availability_period,
scheduling_lookahead                     : pre.scheduling_lookahead,
max_validators_per_core                  : pre.max_validators_per_core,
max_validators                           : pre.max_validators,
dispute_period                           : pre.dispute_period,
dispute_post_conclusion_acceptance_period: pre.dispute_post_conclusion_acceptance_period,
no_show_slots                            : pre.no_show_slots,
n_delay_tranches                         : pre.n_delay_tranches,
zeroth_delay_tranche_width               : pre.zeroth_delay_tranche_width,
needed_approvals                         : pre.needed_approvals,
relay_vrf_modulo_samples                 : pre.relay_vrf_modulo_samples,
ump_max_individual_weight                : pre.ump_max_individual_weight,
pvf_checking_enabled                     : pre.pvf_checking_enabled,
pvf_voting_ttl                           : pre.pvf_voting_ttl,
minimum_validation_upgrade_delay         : pre.minimum_validation_upgrade_delay,
// New fields with safe defaults — async backing disabled, empty executor params.
async_backing_params                     : AsyncBackingParams { max_candidate_depth: 0, allowed_ancestry_len: 0 },
executor_params                          : Default::default(),
		}
	};

	let v4 = v4::ActiveConfig::<T>::get()
		.defensive_proof("Could not decode old config")
		.unwrap_or_default();
	let v5 = translate(v4);
	v5::ActiveConfig::<T>::set(Some(v5));

	// Allowed to be empty.
	let pending_v4 = v4::PendingConfigs::<T>::get().unwrap_or_default();
	let mut pending_v5 = Vec::new();

	for (session, v4) in pending_v4.into_iter() {
		let v5 = translate(v4);
		pending_v5.push((session, v5));
	}
	v5::PendingConfigs::<T>::set(Some(pending_v5.clone()));

	let num_configs = (pending_v5.len() + 1) as u64;
	T::DbWeight::get().reads_writes(num_configs, num_configs)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Test};

	#[test]
	fn test_migrate_to_v5() {
		let v4 = V4HostConfiguration::<primitives::BlockNumber> {
			ump_max_individual_weight: Weight::from_parts(0x71616e6f6e0au64, 0x71616e6f6e0au64),
			needed_approvals: 69,
			thread_availability_period: 55,
			hrmp_recipient_deposit: 1337,
			max_pov_size: 1111,
			chain_availability_period: 33,
			minimum_validation_upgrade_delay: 20,
			..Default::default()
		};

		let mut pending_configs = Vec::new();
		pending_configs.push((100, v4.clone()));
		pending_configs.push((300, v4.clone()));

		new_test_ext(Default::default()).execute_with(|| {
			// Implant the v4 version in the state.
			v4::ActiveConfig::<Test>::set(Some(v4));
			v4::PendingConfigs::<Test>::set(Some(pending_configs));

			migrate_to_v5::<Test>();

			let v5 = v5::ActiveConfig::<Test>::get().unwrap();
			let mut configs_to_check = v5::PendingConfigs::<Test>::get().unwrap();
			configs_to_check.push((0, v5.clone()));

			for (_, v5) in configs_to_check {
				#[rustfmt::skip]
				{
					// Carried-over fields
					assert_eq!(v5.needed_approvals                         , 69);
					assert_eq!(v5.thread_availability_period               , 55);
					assert_eq!(v5.hrmp_recipient_deposit                   , 1337);
					assert_eq!(v5.max_pov_size                             , 1111);
					assert_eq!(v5.chain_availability_period                , 33);
					assert_eq!(v5.minimum_validation_upgrade_delay         , 20);
					assert_eq!(v5.ump_max_individual_weight, Weight::from_parts(0x71616e6f6e0au64, 0x71616e6f6e0au64));
				};

				// New fields have safe defaults
				assert_eq!(v5.async_backing_params.allowed_ancestry_len, 0);
				assert_eq!(v5.async_backing_params.max_candidate_depth, 0);
				assert_eq!(v5.executor_params, ExecutorParams::new());
			}
		});
	}

	// Test that migration doesn't panic in case there are no pending configurations.
	#[test]
	fn test_migrate_to_v5_no_pending() {
		let v4 = V4HostConfiguration::<primitives::BlockNumber>::default();

		new_test_ext(Default::default()).execute_with(|| {
			v4::ActiveConfig::<Test>::set(Some(v4));
			v4::PendingConfigs::<Test>::set(None);

			// Shouldn't fail.
			migrate_to_v5::<Test>();
		});
	}
}
