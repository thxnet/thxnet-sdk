// Copyright 2019-2020 Parity Technologies (UK) Ltd.
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

#![cfg_attr(not(feature = "std"), no_std)]

pub mod weights;

pub use self::currency::DOLLARS;

/// Money matters.
pub mod currency {
	use primitives::Balance;

	pub const UNITS: Balance = 10_000_000_000;
	pub const DOLLARS: Balance = UNITS; // 10_000_000_000
	pub const CENTS: Balance = DOLLARS / 100; // 100_000_000
	pub const MILLICENTS: Balance = CENTS / 1_000; // 100_000

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 20 * DOLLARS + (bytes as Balance) * 100 * MILLICENTS
	}
}

/// Time and blocks.
pub mod time {
	use primitives::{BlockNumber, Moment};
	use runtime_common::prod_or_fast;
	pub const MILLISECS_PER_BLOCK: Moment = 6000;
	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;
	pub const EPOCH_DURATION_IN_SLOTS: BlockNumber = prod_or_fast!(1 * HOURS, 2 * MINUTES);

	// These time units are defined in number of blocks.
	pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;
	pub const WEEKS: BlockNumber = DAYS * 7;

	// 1 in 4 blocks (on average, not counting collisions) will be primary babe blocks.
	// The choice of is done in accordance to the slot duration and expected target
	// block time, for safely resisting network delays of maximum two seconds.
	// <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
	pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);
}

/// Fee-related.
pub mod fee {
	use crate::currency::*;
	use frame_support::weights::{
		WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
	};
	use primitives::Balance;
	use smallvec::smallvec;
	pub use sp_runtime::Perbill;

	pub const INDEX_DEPOSIT: Balance = 0 * DOLLARS;
	pub const EXISTENTIAL_DEPOSIT: Balance = DOLLARS / 1000;
	pub const TRANSACTION_BYTE_FEE: Balance = 0u128;
	pub const OPERATIONAL_FEE_MULTIPLIER: u8 = 0u8;

	/// The block saturation level. Fees will be updates based on this value.
	pub const TARGET_BLOCK_FULLNESS: Perbill = Perbill::from_percent(25);

	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_percent(0),
				coeff_integer: 0,
			}]
		}
	}
}

/// XCM protocol related constants.
pub mod xcm {
	/// Pluralistic bodies existing within the consensus.
	pub mod body {
		// Preallocated for the Root body.
		#[allow(dead_code)]
		const ROOT_INDEX: u32 = 0;
		// The bodies corresponding to the Polkadot OpenGov Origins.
		pub const FELLOWSHIP_ADMIN_INDEX: u32 = 1;
	}
}

/// Staking related.
pub mod staking {

	use primitives::AccountId;

	pub fn get_root_id() -> AccountId {
		array_bytes::hex_n_into_unchecked(
			// 5GL2teb1jKjHgenowY4Y6EQvHkpRHJUpQCYVBDAXk4SADAib
			"bca1b0834cf3b7b0d9258e7a61e5169b16aabfd9233685bfaa8d15c8726b566f",
		)
	}

	pub fn get_reward_id() -> AccountId {
		array_bytes::hex_n_into_unchecked(
			// 5D22dYGvG7ucZZBvFJRQQwKfSKK7LtuiUTshtC3UAukQz7RD
			"2a31b1e8908eb70be0a2688991189bdf4dda1732a43bd73d1ed6482d40343839",
		)
	}
}
