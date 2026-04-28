// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Cumulus.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

use crate::{
	Config, HostConfiguration, Pallet, ReservedDmpWeightOverride, ReservedXcmpWeightOverride,
};
use frame_support::{
	pallet_prelude::*,
	traits::{Get, OnRuntimeUpgrade, StorageVersion},
	weights::Weight,
};

/// The in-code storage version.
pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(3);

/// Migrates the pallet storage to the most recent version.
pub struct Migration<T: Config>(PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for Migration<T> {
	fn on_runtime_upgrade() -> Weight {
		let mut weight: Weight = T::DbWeight::get().reads(2);

		if StorageVersion::get::<Pallet<T>>() == 0 {
			weight = weight
				.saturating_add(v1::migrate::<T>())
				.saturating_add(T::DbWeight::get().writes(1));
			StorageVersion::new(1).put::<Pallet<T>>();
		}

		if StorageVersion::get::<Pallet<T>>() == 1 {
			weight = weight
				.saturating_add(v2::migrate::<T>())
				.saturating_add(T::DbWeight::get().writes(1));
			StorageVersion::new(2).put::<Pallet<T>>();
		}

		if StorageVersion::get::<Pallet<T>>() == 2 {
			weight = weight
				.saturating_add(v3::migrate::<T>())
				.saturating_add(T::DbWeight::get().writes(1));
			StorageVersion::new(3).put::<Pallet<T>>();
		}

		weight
	}
}

/// V2: Migrate to 2D weights for ReservedXcmpWeightOverride and ReservedDmpWeightOverride.
mod v2 {
	use super::*;
	const DEFAULT_POV_SIZE: u64 = 64 * 1024; // 64 KB

	pub fn migrate<T: Config>() -> Weight {
		let translate = |pre: u64| -> Weight { Weight::from_parts(pre, DEFAULT_POV_SIZE) };

		if ReservedXcmpWeightOverride::<T>::translate(|pre| pre.map(translate)).is_err() {
			log::error!(
				target: "parachain_system",
				"unexpected error when performing translation of the ReservedXcmpWeightOverride type during storage upgrade to v2"
			);
		}

		if ReservedDmpWeightOverride::<T>::translate(|pre| pre.map(translate)).is_err() {
			log::error!(
				target: "parachain_system",
				"unexpected error when performing translation of the ReservedDmpWeightOverride type during storage upgrade to v2"
			);
		}

		T::DbWeight::get().reads_writes(2, 2)
	}
}

/// V3: Migrate `HostConfiguration` storage from v0.9.40 shape (9 fields) to v1.12.0 shape
/// (10 fields), appending `async_backing_params` with the disabled-async-backing default
/// `{ max_candidate_depth: 0, allowed_ancestry_len: 0 }`.
///
/// `AsyncBackingParams` does not implement `Default`; the literal is required.
/// Field assignment order in the translate closure mirrors the v0.9.40 declaration order
/// (Section 2 of the Wave-A design memo) to satisfy SCALE's positional decode contract.
mod v3 {
	use super::*;
	use cumulus_primitives_core::{relay_chain, AbridgedHostConfiguration};

	/// Old (v0.9.40) shape of `AbridgedHostConfiguration` — used only for SCALE decoding
	/// during the v2→v3 storage migration. Field names, types, and order are byte-identical
	/// to `polkadot v0.9.40 primitives/src/v2/mod.rs:1049`.
	#[derive(Encode, Decode, RuntimeDebug)]
	pub struct OldAbridgedHostConfiguration {
		pub max_code_size: u32,
		pub max_head_data_size: u32,
		pub max_upward_queue_count: u32,
		pub max_upward_queue_size: u32,
		pub max_upward_message_size: u32,
		pub max_upward_message_num_per_candidate: u32,
		pub hrmp_max_message_num_per_candidate: u32,
		pub validation_upgrade_cooldown: relay_chain::BlockNumber,
		pub validation_upgrade_delay: relay_chain::BlockNumber,
	}

	pub fn migrate<T: Config>() -> Weight {
		if HostConfiguration::<T>::translate(|pre: Option<OldAbridgedHostConfiguration>| {
			pre.map(|old| AbridgedHostConfiguration {
				// Fields 1–9: copied 1:1 in v0.9.40 declaration order.
				// Reordering any of these would corrupt on-chain data.
				max_code_size: old.max_code_size,
				max_head_data_size: old.max_head_data_size,
				max_upward_queue_count: old.max_upward_queue_count,
				max_upward_queue_size: old.max_upward_queue_size,
				max_upward_message_size: old.max_upward_message_size,
				max_upward_message_num_per_candidate: old.max_upward_message_num_per_candidate,
				hrmp_max_message_num_per_candidate: old.hrmp_max_message_num_per_candidate,
				validation_upgrade_cooldown: old.validation_upgrade_cooldown,
				validation_upgrade_delay: old.validation_upgrade_delay,
				// Field 10: NEW in v1.12.0 — async backing was not present in v0.9.40.
				// `AsyncBackingParams` does not impl Default; literal required.
				// Value 0/0 is the only safe value when async backing is disabled.
				async_backing_params: relay_chain::AsyncBackingParams {
					max_candidate_depth: 0,
					allowed_ancestry_len: 0,
				},
			})
		})
		.is_err()
		{
			log::error!(
				target: "parachain_system",
				"unexpected error when migrating HostConfiguration to v3"
			);
		}

		T::DbWeight::get().reads_writes(1, 1)
	}
}

#[cfg(test)]
mod migration_tests {
	use super::*;
	use crate::mock::{new_test_ext, Test};
	use codec::Encode;

	/// Sentinel values chosen to be non-zero and mutually distinct so that a
	/// field-swap bug would surface as a wrong value rather than an accidental
	/// match against 0.
	const S_MAX_CODE_SIZE: u32 = 1_000_001;
	const S_MAX_HEAD_DATA_SIZE: u32 = 1_000_002;
	const S_MAX_UPWARD_QUEUE_COUNT: u32 = 1_000_003;
	const S_MAX_UPWARD_QUEUE_SIZE: u32 = 1_000_004;
	const S_MAX_UPWARD_MESSAGE_SIZE: u32 = 1_000_005;
	const S_MAX_UPWARD_MESSAGE_NUM_PER_CANDIDATE: u32 = 1_000_006;
	const S_HRMP_MAX_MESSAGE_NUM_PER_CANDIDATE: u32 = 1_000_007;
	const S_VALIDATION_UPGRADE_COOLDOWN: u32 = 1_000_008;
	const S_VALIDATION_UPGRADE_DELAY: u32 = 1_000_009;

	/// Build a raw SCALE-encoded v0.9.40 `AbridgedHostConfiguration` (9 fields)
	/// and return the bytes. The caller injects these directly into storage to
	/// simulate a pre-migration node's on-chain state.
	fn old_config_raw_bytes() -> Vec<u8> {
		v3::OldAbridgedHostConfiguration {
			max_code_size: S_MAX_CODE_SIZE,
			max_head_data_size: S_MAX_HEAD_DATA_SIZE,
			max_upward_queue_count: S_MAX_UPWARD_QUEUE_COUNT,
			max_upward_queue_size: S_MAX_UPWARD_QUEUE_SIZE,
			max_upward_message_size: S_MAX_UPWARD_MESSAGE_SIZE,
			max_upward_message_num_per_candidate: S_MAX_UPWARD_MESSAGE_NUM_PER_CANDIDATE,
			hrmp_max_message_num_per_candidate: S_HRMP_MAX_MESSAGE_NUM_PER_CANDIDATE,
			validation_upgrade_cooldown: S_VALIDATION_UPGRADE_COOLDOWN,
			validation_upgrade_delay: S_VALIDATION_UPGRADE_DELAY,
		}
		.encode()
	}

	#[test]
	fn v3_migrate_round_trips_raw_v0940_bytes_and_zero_fills_async_backing() {
		new_test_ext().execute_with(|| {
			// --- ARRANGE ---
			// Inject the 9-field (v0.9.40) SCALE blob directly into the
			// `HostConfiguration` storage slot, bypassing the v1.12.0 SCALE layer.
			// This is the real pre-migration state: 9 u32 words, no async_backing_params.
			let raw_bytes = old_config_raw_bytes();
			sp_io::storage::set(&HostConfiguration::<Test>::hashed_key(), &raw_bytes);

			// --- ACT ---
			v3::migrate::<Test>();

			// --- ASSERT ---
			let cfg = HostConfiguration::<Test>::get()
				.expect("HostConfiguration must be Some after v3 migration");

			// Fields 1–9: must be carried through verbatim.
			assert_eq!(cfg.max_code_size, S_MAX_CODE_SIZE, "max_code_size");
			assert_eq!(cfg.max_head_data_size, S_MAX_HEAD_DATA_SIZE, "max_head_data_size");
			assert_eq!(
				cfg.max_upward_queue_count, S_MAX_UPWARD_QUEUE_COUNT,
				"max_upward_queue_count"
			);
			assert_eq!(cfg.max_upward_queue_size, S_MAX_UPWARD_QUEUE_SIZE, "max_upward_queue_size");
			assert_eq!(
				cfg.max_upward_message_size, S_MAX_UPWARD_MESSAGE_SIZE,
				"max_upward_message_size"
			);
			assert_eq!(
				cfg.max_upward_message_num_per_candidate, S_MAX_UPWARD_MESSAGE_NUM_PER_CANDIDATE,
				"max_upward_message_num_per_candidate"
			);
			assert_eq!(
				cfg.hrmp_max_message_num_per_candidate, S_HRMP_MAX_MESSAGE_NUM_PER_CANDIDATE,
				"hrmp_max_message_num_per_candidate"
			);
			assert_eq!(
				cfg.validation_upgrade_cooldown, S_VALIDATION_UPGRADE_COOLDOWN,
				"validation_upgrade_cooldown"
			);
			assert_eq!(
				cfg.validation_upgrade_delay, S_VALIDATION_UPGRADE_DELAY,
				"validation_upgrade_delay"
			);

			// Field 10 (NEW): async_backing_params must be 0/0 (disabled-async-backing default).
			assert_eq!(
				cfg.async_backing_params.max_candidate_depth, 0,
				"async_backing_params.max_candidate_depth"
			);
			assert_eq!(
				cfg.async_backing_params.allowed_ancestry_len, 0,
				"async_backing_params.allowed_ancestry_len"
			);
		});
	}

	#[test]
	fn v3_migrate_is_idempotent_when_storage_is_absent() {
		new_test_ext().execute_with(|| {
			// If HostConfiguration is None (e.g. genesis not yet written), migrate
			// must not panic and must leave storage as None.
			assert!(HostConfiguration::<Test>::get().is_none());
			v3::migrate::<Test>();
			assert!(HostConfiguration::<Test>::get().is_none(), "None in → None out");
		});
	}

	/// Integration test: exercises the full `Migration::<Test>::on_runtime_upgrade()` path
	/// (not `v3::migrate` in isolation). Verifies that:
	/// (a) on-chain StorageVersion advances from 2 → 3,
	/// (b) all 9 sentinel fields from the old v0.9.40 blob are decoded verbatim,
	/// (c) the new `async_backing_params` field is zero-filled.
	#[test]
	fn on_runtime_upgrade_v2_to_v3_bumps_version_and_decodes_all_fields() {
		new_test_ext().execute_with(|| {
			// ── ARRANGE ──
			// Simulate a node whose pallet storage is at on-chain version 2
			// (i.e. it has already run migrations v0→1 and v1→2).
			StorageVersion::new(2).put::<Pallet<Test>>();
			assert_eq!(
				StorageVersion::get::<Pallet<Test>>(),
				StorageVersion::new(2),
				"pre-condition: StorageVersion must be 2"
			);

			// Inject the 9-field (v0.9.40) SCALE blob directly into the
			// `HostConfiguration` storage slot — exactly what a real chain carries
			// before this migration runs.
			let raw_bytes = old_config_raw_bytes();
			sp_io::storage::set(&HostConfiguration::<Test>::hashed_key(), &raw_bytes);

			// ── ACT ──
			// Call the top-level Migration impl — this is the entry-point that
			// frame-executive invokes. It must skip versions 0→1 and 1→2 (already done)
			// and apply only the 2→3 step.
			Migration::<Test>::on_runtime_upgrade();

			// ── ASSERT (a): StorageVersion advanced to 3 ──
			assert_eq!(
				StorageVersion::get::<Pallet<Test>>(),
				StorageVersion::new(3),
				"StorageVersion must be 3 after on_runtime_upgrade"
			);

			// ── ASSERT (b): all 9 sentinel fields decoded verbatim ──
			let cfg = HostConfiguration::<Test>::get()
				.expect("HostConfiguration must be Some after v3 migration");

			assert_eq!(cfg.max_code_size, S_MAX_CODE_SIZE, "max_code_size");
			assert_eq!(cfg.max_head_data_size, S_MAX_HEAD_DATA_SIZE, "max_head_data_size");
			assert_eq!(
				cfg.max_upward_queue_count, S_MAX_UPWARD_QUEUE_COUNT,
				"max_upward_queue_count"
			);
			assert_eq!(cfg.max_upward_queue_size, S_MAX_UPWARD_QUEUE_SIZE, "max_upward_queue_size");
			assert_eq!(
				cfg.max_upward_message_size, S_MAX_UPWARD_MESSAGE_SIZE,
				"max_upward_message_size"
			);
			assert_eq!(
				cfg.max_upward_message_num_per_candidate, S_MAX_UPWARD_MESSAGE_NUM_PER_CANDIDATE,
				"max_upward_message_num_per_candidate"
			);
			assert_eq!(
				cfg.hrmp_max_message_num_per_candidate, S_HRMP_MAX_MESSAGE_NUM_PER_CANDIDATE,
				"hrmp_max_message_num_per_candidate"
			);
			assert_eq!(
				cfg.validation_upgrade_cooldown, S_VALIDATION_UPGRADE_COOLDOWN,
				"validation_upgrade_cooldown"
			);
			assert_eq!(
				cfg.validation_upgrade_delay, S_VALIDATION_UPGRADE_DELAY,
				"validation_upgrade_delay"
			);

			// ── ASSERT (c): async_backing_params zero-filled ──
			assert_eq!(
				cfg.async_backing_params.max_candidate_depth, 0,
				"async_backing_params.max_candidate_depth must be 0 (disabled)"
			);
			assert_eq!(
				cfg.async_backing_params.allowed_ancestry_len, 0,
				"async_backing_params.allowed_ancestry_len must be 0 (disabled)"
			);
		});
	}
}

/// V1: `LastUpgrade` block number is removed from the storage since the upgrade
/// mechanism now uses signals instead of block offsets.
mod v1 {
	use crate::{Config, Pallet};
	#[allow(deprecated)]
	use frame_support::{migration::remove_storage_prefix, pallet_prelude::*};

	pub fn migrate<T: Config>() -> Weight {
		#[allow(deprecated)]
		remove_storage_prefix(<Pallet<T>>::name().as_bytes(), b"LastUpgrade", b"");
		T::DbWeight::get().writes(1)
	}
}
