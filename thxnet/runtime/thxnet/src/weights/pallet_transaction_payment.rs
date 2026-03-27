// Stub weights for pallet_transaction_payment.
// Auto-generated placeholder — replace with benchmark results.

#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::Weight};

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_transaction_payment::WeightInfo for WeightInfo<T> {
	fn charge_transaction_payment() -> Weight {
		Weight::zero()
	}
}
