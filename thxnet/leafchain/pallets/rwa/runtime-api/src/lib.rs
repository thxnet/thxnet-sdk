#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use codec::Codec;
use pallet_rwa::{CanParticipateError, ParticipationStatus};

sp_api::decl_runtime_apis! {
	pub trait RwaApi<AccountId, Balance, BlockNumber, AssetId>
	where
		AccountId: Codec,
		Balance: Codec,
		BlockNumber: Codec,
		AssetId: Codec,
	{
		fn effective_participation_status(
			asset_id: u32,
			participation_id: u32,
		) -> Option<ParticipationStatus<BlockNumber>>;

		fn can_participate(
			asset_id: u32,
			who: AccountId,
		) -> Result<(), CanParticipateError>;

		fn assets_by_owner(owner: AccountId) -> Vec<u32>;

		fn participations_by_holder(holder: AccountId) -> Vec<(u32, u32)>;

		fn active_participant_count(asset_id: u32) -> u32;
	}
}
