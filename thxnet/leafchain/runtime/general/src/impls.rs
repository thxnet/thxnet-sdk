use frame_support::traits::fungibles::{Balanced, Credit};
use pallet_asset_tx_payment::HandleCredit;
use pallet_crowdfunding::CampaignStatus;
use pallet_rwa::{AssetLifecycleGuard, ParticipationStatus};
use sp_runtime::DispatchResult;

use crate::{AccountId, Assets, BlockNumber, Runtime};

/// A `HandleCredit` implementation that naively transfers the fees to the block
/// author. Will drop and burn the assets in case the transfer fails.
pub struct CreditToBlockAuthor;

impl HandleCredit<AccountId, Assets> for CreditToBlockAuthor {
	fn handle_credit(credit: Credit<AccountId, Assets>) {
		if let Some(author) = pallet_authorship::Pallet::<Runtime>::author() {
			// Drop the result which will trigger the `OnDrop` of the imbalance in case of
			// error.
			let _ = <Assets as Balanced<AccountId>>::resolve(&author, credit);
		}
	}
}

/// Cross-pallet guard: prevents RWA asset retirement/slashing when active
/// crowdfunding campaigns are linked to it.
///
/// CRIT-03 fix: without this guard, an admin `force_retire_asset` would
/// cascade to ALL linked campaigns via `report_license_revoked`, potentially
/// locking investor funds.
pub struct CrowdfundingLifecycleGuard;

impl AssetLifecycleGuard<AccountId> for CrowdfundingLifecycleGuard {
	fn can_retire_asset(rwa_asset_id: u32) -> DispatchResult {
		// Check if any active campaign references this RWA asset.
		// A campaign is "active" if its status is Funding, Paused, or MilestonePhase.
		let has_active = pallet_crowdfunding::Campaigns::<Runtime>::iter_values().any(|c| {
			c.rwa_asset_id == Some(rwa_asset_id) &&
				matches!(
					c.status,
					CampaignStatus::Funding |
						CampaignStatus::Paused | CampaignStatus::MilestonePhase
				)
		});
		if has_active {
			return Err(sp_runtime::DispatchError::Other(
				"Cannot retire: active campaigns linked to this RWA asset",
			));
		}
		Ok(())
	}

	fn can_slash_participation(rwa_asset_id: u32, _participation_id: u32) -> DispatchResult {
		// Same logic: block slashing if any active campaign is linked.
		let has_active = pallet_crowdfunding::Campaigns::<Runtime>::iter_values().any(|c| {
			c.rwa_asset_id == Some(rwa_asset_id) &&
				matches!(
					c.status,
					CampaignStatus::Funding |
						CampaignStatus::Paused | CampaignStatus::MilestonePhase
				)
		});
		if has_active {
			return Err(sp_runtime::DispatchError::Other(
				"Cannot slash: active campaigns linked to this RWA asset",
			));
		}
		Ok(())
	}
}

/// Cross-pallet verifier: checks pallet-rwa for active participation
/// before allowing campaign creation with a license requirement.
///
/// This is the production implementation of `LicenseVerifier` that connects
/// pallet-crowdfunding to pallet-rwa, enabling license-gated campaigns.
pub struct RwaLicenseVerifier;

impl pallet_crowdfunding::LicenseVerifier<AccountId, BlockNumber> for RwaLicenseVerifier {
	fn ensure_active_license(
		rwa_asset_id: u32,
		participation_id: u32,
		who: &AccountId,
	) -> DispatchResult {
		// Check asset exists and is Active.
		let asset = pallet_rwa::RwaAssets::<Runtime>::get(rwa_asset_id)
			.ok_or(sp_runtime::DispatchError::Other("RWA asset not found"))?;
		if !matches!(asset.status, pallet_rwa::AssetStatus::Active) {
			return Err(sp_runtime::DispatchError::Other("RWA asset not active"));
		}

		// Check participation exists and is Active.
		let participation =
			pallet_rwa::Participations::<Runtime>::get(rwa_asset_id, participation_id)
				.ok_or(sp_runtime::DispatchError::Other("Participation not found"))?;
		match &participation.status {
			ParticipationStatus::Active { .. } => {},
			_ => return Err(sp_runtime::DispatchError::Other("Participation not active")),
		}

		// Check caller is a holder.
		if !participation.holders.iter().any(|h| h == who) {
			return Err(sp_runtime::DispatchError::Other("Not a holder of this participation"));
		}

		Ok(())
	}

	fn is_license_active(rwa_asset_id: u32, participation_id: u32) -> bool {
		// Check asset is Active.
		let Some(asset) = pallet_rwa::RwaAssets::<Runtime>::get(rwa_asset_id) else {
			return false;
		};
		if !matches!(asset.status, pallet_rwa::AssetStatus::Active) {
			return false;
		}

		// Check participation is Active and not expired.
		let Some(participation) =
			pallet_rwa::Participations::<Runtime>::get(rwa_asset_id, participation_id)
		else {
			return false;
		};
		match participation.status {
			ParticipationStatus::Active { expires_at, .. } => {
				if let Some(expiry) = expires_at {
					let now = frame_system::Pallet::<Runtime>::block_number();
					now < expiry
				} else {
					true // No expiry = always active
				}
			},
			_ => false,
		}
	}

	fn license_expiry(rwa_asset_id: u32, participation_id: u32) -> Option<BlockNumber> {
		let participation =
			pallet_rwa::Participations::<Runtime>::get(rwa_asset_id, participation_id)?;
		match participation.status {
			ParticipationStatus::Active { expires_at, .. } => expires_at,
			_ => None,
		}
	}
}
