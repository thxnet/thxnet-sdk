//! Tests for DAO pallet.

use frame_support::{
	construct_runtime,
	traits::{ConstU128, ConstU32, ConstU64},
};
use sp_core::H256;
use sp_keystore::{testing::MemoryKeystore, KeystoreExt};
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

use crate::{self as pallet_dao};

type Block = frame_system::mocking::MockBlock<Test>;

pub const UNITS: u128 = 10_000_000_000;

construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Timestamp: pallet_timestamp,
		Dao: pallet_dao,
	}
);

impl frame_system::Config for Test {
	type AccountData = pallet_balances::AccountData<u128>;
	type AccountId = u64;
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockHashCount = ConstU64<250>;
	type BlockLength = ();
	type Block = Block;
	type BlockWeights = ();
	type DbWeight = ();
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = ConstU32<16>;
	type Nonce = u64;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type PalletInfo = PalletInfo;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type SS58Prefix = ();
	type SystemWeightInfo = ();
	type Version = ();
}

impl pallet_balances::Config for Test {
	type AccountStore = System;
	type Balance = u128;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<1>;
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxHolds = ConstU32<0>;
	type MaxFreezes = ConstU32<0>;
}

impl pallet_timestamp::Config for Test {
	type MinimumPeriod = ConstU64<1>;
	type Moment = u64;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

impl pallet_dao::Config for Test {
	type Currency = Balances;
	type CurrencyUnits = ConstU128<{ UNITS }>;
	type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
	type OptionIndex = u64;
	type RuntimeEvent = RuntimeEvent;
	type StringLimit = ConstU32<{ 4 * 2048 }>;
	type TopicDescriptionMaximumLength = ConstU32<2048>;
	type TopicDescriptionMinimumLength = ConstU32<1>;
	type TopicId = u64;
	type TopicOptionMaximumLength = ConstU32<256>;
	type TopicOptionMaximumNumber = ConstU32<1024>;
	type TopicOptionMinimumLength = ConstU32<1>;
	type TopicRaiserBalanceLowerBound = ConstU128<{ 1_000_000 * UNITS }>;
	type TopicTitleMaximumLength = ConstU32<256>;
	type TopicTitleMinimumLength = ConstU32<1>;
	type UnixTime = pallet_timestamp::Pallet<Test>;
	type Vote = u128;
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let balance = 1_000_000 * UNITS;
	pallet_balances::pallet::GenesisConfig::<Test> {
		balances: (0..100).map(|i| (i, balance * UNITS)).collect(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let keystore = MemoryKeystore::new();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.register_extension(KeystoreExt::new(keystore));
	ext.execute_with(|| System::set_block_number(6));
	ext
}
