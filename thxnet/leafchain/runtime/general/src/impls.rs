use frame_support::traits::fungibles::{Balanced, Credit};
use pallet_asset_tx_payment::HandleCredit;

use crate::{AccountId, Assets, Runtime};

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
