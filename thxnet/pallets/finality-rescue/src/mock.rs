use frame_support::{
	construct_runtime, derive_impl, parameter_types,
	traits::{ConstU32, ConstU64},
};
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
	pub enum Test {
		System: frame_system,
		Grandpa: pallet_grandpa,
		FinalityRescue: crate,
	}
);

parameter_types! {
	pub const RescueCooldown: u64 = 10;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
}

impl pallet_grandpa::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type MaxAuthorities = ConstU32<100>;
	type MaxNominators = ConstU32<100>;
	type MaxSetIdSessionEntries = ConstU64<0>;
	type KeyOwnerProof = sp_core::Void;
	type EquivocationReportSystem = ();
}

impl crate::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RescueCooldown = RescueCooldown;
}

pub fn test_authorities() -> Vec<(pallet_grandpa::AuthorityId, u64)> {
	use sp_core::crypto::UncheckedFrom;
	vec![
		(pallet_grandpa::AuthorityId::unchecked_from([1u8; 32]), 1),
		(pallet_grandpa::AuthorityId::unchecked_from([2u8; 32]), 1),
		(pallet_grandpa::AuthorityId::unchecked_from([3u8; 32]), 1),
	]
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	pallet_grandpa::GenesisConfig::<Test> { authorities: test_authorities(), _config: Default::default() }
		.assimilate_storage(&mut t)
		.unwrap();

	t.into()
}

pub fn new_test_ext_no_authorities() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::<Test>::default().build_storage().unwrap().into()
}
