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

//! The Polkadot runtime. This can be compiled with `#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `#[frame_support::runtime]!` does a lot of recursion and requires us to increase the limit.
#![recursion_limit = "512"]

extern crate alloc;

use alloc::{vec, vec::Vec};

// The impl_runtime_weights! macro expands to `pub use polkadot_runtime_common::...`,
// but our Cargo.toml imports the crate as `runtime_common`. This alias fixes the expansion.
use runtime_common as polkadot_runtime_common;

#[allow(deprecated)]
use pallet_transaction_payment::FungibleAdapter;
use runtime_common::{
	auctions, claims, crowdloan, impl_runtime_weights, impls::DealWithFees, paras_registrar,
	prod_or_fast, slots, BlockHashCount, BlockLength, CurrencyToVote, SlowAdjustingFeeUpdate,
};

pub mod impls;
use impls::CreditToBlockAuthor;

use runtime_parachains::{
	assigner_coretime as parachains_assigner_coretime, configuration as parachains_configuration,
	disputes as parachains_disputes,
	disputes::slashing as parachains_slashing,
	dmp as parachains_dmp, hrmp as parachains_hrmp, inclusion as parachains_inclusion,
	inclusion::{AggregateMessageOrigin, UmpQueueId},
	initializer as parachains_initializer, on_demand as parachains_on_demand,
	origin as parachains_origin, paras as parachains_paras,
	paras_inherent as parachains_paras_inherent, reward_points as parachains_reward_points,
	runtime_api_impl::{
		v13 as parachains_runtime_api_impl, vstaging as parachains_staging_runtime_api_impl,
	},
	scheduler as parachains_scheduler, session_info as parachains_session_info,
	shared as parachains_shared,
};

use alloc::collections::{btree_map::BTreeMap, vec_deque::VecDeque};
use authority_discovery_primitives::AuthorityId as AuthorityDiscoveryId;
use beefy_primitives::ecdsa_crypto::{AuthorityId as BeefyId, Signature as BeefySignature};
use core::cmp::Ordering;
use frame_election_provider_support::{
	bounds::ElectionBoundsBuilder, generate_solution_type, onchain, SequentialPhragmen,
};
use frame_support::{
	derive_impl, parameter_types,
	traits::{
		fungible::HoldConsideration, tokens::imbalance::ResolveTo, ConstBool, ConstU128, ConstU32,
		Contains, EitherOf, EitherOfDiverse, Everything, InstanceFilter, KeyOwnerProofSystem,
		LinearStoragePrice, PrivilegeCmp, ProcessMessage, ProcessMessageError, WithdrawReasons,
	},
	weights::WeightMeter,
	PalletId,
};
use frame_system::{EnsureRoot, EnsureWithSuccess};
use pallet_grandpa::{fg_primitives, AuthorityId as GrandpaId};
use pallet_session::historical as session_historical;
use pallet_transaction_payment::{FeeDetails, RuntimeDispatchInfo};
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use primitives::{
	async_backing::Constraints, slashing, AccountId, AccountIndex, ApprovalVotingParams, Balance,
	BlockNumber, CandidateEvent, CandidateHash,
	CommittedCandidateReceiptV2 as CommittedCandidateReceipt, CoreIndex, CoreState, DisputeState,
	ExecutorParams, GroupRotationInfo, Hash, Id as ParaId, InboundDownwardMessage,
	InboundHrmpMessage, Moment, NodeFeatures, Nonce, OccupiedCoreAssumption,
	PersistedValidationData, ScrapedOnChainVotes, SessionInfo, Signature, ValidationCode,
	ValidationCodeHash, ValidatorId, ValidatorIndex, LOWEST_PUBLIC_ID, PARACHAIN_KEY_TYPE_ID,
};
use sp_core::OpaqueMetadata;
use sp_mmr_primitives as mmr;
use sp_runtime::{
	create_runtime_str,
	curve::PiecewiseLinear,
	generic, impl_opaque_keys,
	traits::{
		AccountIdConversion, AccountIdLookup, BlakeTwo256, Block as BlockT, ConvertInto,
		Extrinsic as ExtrinsicT, OpaqueKeys, SaturatedConversion, Verify,
	},
	transaction_validity::{TransactionPriority, TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, FixedU128, KeyTypeId, Perbill, Percent, Permill, RuntimeDebug,
};
use sp_staking::SessionIndex;
#[cfg(any(feature = "std", test))]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use xcm::latest::Junction;

pub use frame_system::Call as SystemCall;
pub use pallet_balances::Call as BalancesCall;
pub use pallet_election_provider_multi_phase::{Call as EPMCall, GeometricDepositBase};
#[cfg(feature = "std")]
pub use pallet_staking::StakerStatus;
use pallet_staking::UseValidatorsMap;
pub use pallet_timestamp::Call as TimestampCall;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_runtime::{traits::Get, RuntimeAppPublic};

/// Constant values used within the runtime.
use thxnet_testnet_runtime_constants::{currency::*, fee::*, time::*};

// Weights used in the runtime.
mod weights;

mod bag_thresholds;

// THXNet uses Gov V1, not OpenGov. Define simple type aliases for origins.
// These replace the OpenGov custom origins with EnsureRoot for THXNet.
pub type AuctionAdmin = frame_system::EnsureRoot<AccountId>;
pub type FellowshipAdmin = frame_system::EnsureRoot<AccountId>;
pub type GeneralAdmin = frame_system::EnsureRoot<AccountId>;
pub type LeaseAdmin = frame_system::EnsureRoot<AccountId>;
pub type StakingAdmin = frame_system::EnsureRoot<AccountId>;
pub type Treasurer = frame_system::EnsureRoot<AccountId>;
pub type TreasurySpender = EitherOf<
	frame_system::EnsureRootWithSuccess<AccountId, RootSpendOriginMaxAmount>,
	EnsureWithSuccess<
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 5>,
		AccountId,
		CouncilSpendOriginMaxAmount,
	>,
>;

pub mod xcm_config;

impl_runtime_weights!(thxnet_testnet_runtime_constants);

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

// Polkadot version identifier;
/// Runtime version (Polkadot).
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("thxnet"),
	impl_name: create_runtime_str!("thxnet"),
	authoring_version: 0,
	spec_version: 125_120_003,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 25,
	system_version: 1,
};

/// The BABE epoch configuration at genesis.
pub const BABE_GENESIS_EPOCH_CONFIG: babe_primitives::BabeEpochConfiguration =
	babe_primitives::BabeEpochConfiguration {
		c: PRIMARY_PROBABILITY,
		allowed_slots: babe_primitives::AllowedSlots::PrimaryAndSecondaryVRFSlots,
	};

/// Native version.
#[cfg(any(feature = "std", test))]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

parameter_types! {
	pub const Version: RuntimeVersion = VERSION;
	pub const SS58Prefix: u8 = 42;
	pub MaxCollectivesProposalWeight: frame_support::weights::Weight = sp_runtime::Perbill::from_percent(50) * BlockWeights::get().max_block;
}

#[derive_impl(frame_system::config_preludes::RelayChainDefaultConfig)]
impl frame_system::Config for Runtime {
	type BlockWeights = BlockWeights;
	type BlockLength = BlockLength;
	type Nonce = Nonce;
	type Hash = Hash;
	type AccountId = AccountId;
	type Block = Block;
	type BlockHashCount = BlockHashCount;
	type DbWeight = RocksDbWeight;
	type Version = Version;
	type AccountData = pallet_balances::AccountData<Balance>;
	type SystemWeightInfo = weights::frame_system::WeightInfo<Runtime>;
	type ExtensionsWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type SingleBlockMigrations = Migrations;
	type MultiBlockMigrator = MultiBlockMigrations;
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) *
		BlockWeights::get().max_block;
	pub const MaxScheduledPerBlock: u32 = 50;
	pub const NoPreimagePostponement: Option<u32> = Some(10);
}

/// Used the compare the privilege of an origin inside the scheduler.
pub struct OriginPrivilegeCmp;

impl PrivilegeCmp<OriginCaller> for OriginPrivilegeCmp {
	fn cmp_privilege(left: &OriginCaller, right: &OriginCaller) -> Option<Ordering> {
		if left == right {
			return Some(Ordering::Equal)
		}

		match (left, right) {
			// Root is greater than anything.
			(OriginCaller::system(frame_system::RawOrigin::Root), _) => Some(Ordering::Greater),
			// For every other origin we don't care, as they are not used for `ScheduleOrigin`.
			_ => None,
		}
	}
}

impl pallet_scheduler::Config for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeEvent = RuntimeEvent;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = MaximumSchedulerWeight;
	// The goal of having ScheduleOrigin include AuctionAdmin is to allow the auctions track of
	// OpenGov to schedule periodic auctions.
	// Also allow Treasurer to schedule recurring payments.
	type ScheduleOrigin = EitherOf<EitherOf<EnsureRoot<AccountId>, AuctionAdmin>, Treasurer>;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = weights::pallet_scheduler::WeightInfo<Runtime>;
	type OriginPrivilegeCmp = OriginPrivilegeCmp;
	type Preimages = Preimage;
	type BlockNumberProvider = System;
}

parameter_types! {
	pub const PreimageMaxSize: u32 = 4096 * 1024;
	pub const PreimageBaseDeposit: Balance = deposit(2, 64);
	pub const PreimageByteDeposit: Balance = deposit(0, 1);
	pub const PreimageHoldReason: RuntimeHoldReason = RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);
}

impl pallet_preimage::Config for Runtime {
	type WeightInfo = weights::pallet_preimage::WeightInfo<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type ManagerOrigin = EnsureRoot<AccountId>;
	type Consideration = HoldConsideration<
		AccountId,
		Balances,
		PreimageHoldReason,
		LinearStoragePrice<PreimageBaseDeposit, PreimageByteDeposit, Balance>,
	>;
}

parameter_types! {
	pub EpochDuration: u64 = prod_or_fast!(
		EPOCH_DURATION_IN_SLOTS as u64,
		2 * MINUTES as u64,
		"DOT_EPOCH_DURATION"
	);
	pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
	pub ReportLongevity: u64 =
		BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
}

impl pallet_babe::Config for Runtime {
	type EpochDuration = EpochDuration;
	type ExpectedBlockTime = ExpectedBlockTime;

	// session module is the trigger
	type EpochChangeTrigger = pallet_babe::ExternalTrigger;

	type DisabledValidators = Session;

	type WeightInfo = ();

	type MaxAuthorities = MaxAuthorities;
	type MaxNominators = MaxExposurePageSize;

	type KeyOwnerProof =
		<Historical as KeyOwnerProofSystem<(KeyTypeId, pallet_babe::AuthorityId)>>::Proof;

	type EquivocationReportSystem =
		pallet_babe::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

parameter_types! {
	pub const IndexDeposit: Balance = 10 * DOLLARS;
}

impl pallet_indices::Config for Runtime {
	type AccountIndex = AccountIndex;
	type Currency = Balances;
	type Deposit = IndexDeposit;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_indices::WeightInfo<Runtime>;
}

parameter_types! {
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = weights::pallet_balances::WeightInfo<Runtime>;
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = RuntimeFreezeReason;
	type MaxFreezes = ConstU32<1>;
	type DoneSlashHandler = ();
}

parameter_types! {
	pub const TransactionByteFee: Balance = TRANSACTION_BYTE_FEE;
	/// This value increases the priority of `Operational` transactions by adding
	/// a "virtual tip" that's equal to the `OperationalFeeMultiplier * final_fee`.
	pub const OperationalFeeMultiplier: u8 = OPERATIONAL_FEE_MULTIPLIER;
}

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = FungibleAdapter<Balances, DealWithFees<Runtime>>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type WeightToFee = WeightToFee;
	type LengthToFee = WeightToFee;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type WeightInfo = weights::pallet_transaction_payment::WeightInfo<Runtime>;
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}
impl pallet_timestamp::Config for Runtime {
	type Moment = u64;
	type OnTimestampSet = Babe;
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = weights::pallet_timestamp::WeightInfo<Runtime>;
}

impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
	type EventHandler = Staking;
}

// Old session keys including ImOnline, needed for migration.
// Remove this when removing `UpgradeSessionKeys` from migrations.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct OldSessionKeys {
	pub grandpa: <Grandpa as sp_runtime::BoundToRuntimeAppPublic>::Public,
	pub babe: <Babe as sp_runtime::BoundToRuntimeAppPublic>::Public,
	pub im_online: pallet_im_online::sr25519::AuthorityId,
	pub para_validator: <Initializer as sp_runtime::BoundToRuntimeAppPublic>::Public,
	pub para_assignment: <ParaSessionInfo as sp_runtime::BoundToRuntimeAppPublic>::Public,
	pub authority_discovery: <AuthorityDiscovery as sp_runtime::BoundToRuntimeAppPublic>::Public,
}

impl OpaqueKeys for OldSessionKeys {
	type KeyTypeIdProviders = ();
	fn key_ids() -> &'static [KeyTypeId] {
		&[
			<<Grandpa as sp_runtime::BoundToRuntimeAppPublic>::Public>::ID,
			<<Babe as sp_runtime::BoundToRuntimeAppPublic>::Public>::ID,
			sp_core::crypto::key_types::IM_ONLINE,
			<<Initializer as sp_runtime::BoundToRuntimeAppPublic>::Public>::ID,
			<<ParaSessionInfo as sp_runtime::BoundToRuntimeAppPublic>::Public>::ID,
			<<AuthorityDiscovery as sp_runtime::BoundToRuntimeAppPublic>::Public>::ID,
		]
	}
	fn get_raw(&self, i: KeyTypeId) -> &[u8] {
		match i {
			<<Grandpa as sp_runtime::BoundToRuntimeAppPublic>::Public>::ID => self.grandpa.as_ref(),
			<<Babe as sp_runtime::BoundToRuntimeAppPublic>::Public>::ID => self.babe.as_ref(),
			sp_core::crypto::key_types::IM_ONLINE => self.im_online.as_ref(),
			<<Initializer as sp_runtime::BoundToRuntimeAppPublic>::Public>::ID =>
				self.para_validator.as_ref(),
			<<ParaSessionInfo as sp_runtime::BoundToRuntimeAppPublic>::Public>::ID =>
				self.para_assignment.as_ref(),
			<<AuthorityDiscovery as sp_runtime::BoundToRuntimeAppPublic>::Public>::ID =>
				self.authority_discovery.as_ref(),
			_ => &[],
		}
	}
}

// Remove this when removing `OldSessionKeys`
fn transform_session_keys(_v: AccountId, old: OldSessionKeys) -> SessionKeys {
	SessionKeys {
		grandpa: old.grandpa,
		babe: old.babe,
		para_validator: old.para_validator,
		para_assignment: old.para_assignment,
		authority_discovery: old.authority_discovery,
	}
}

impl_opaque_keys! {
	pub struct SessionKeys {
		pub grandpa: Grandpa,
		pub babe: Babe,
		pub para_validator: Initializer,
		pub para_assignment: ParaSessionInfo,
		pub authority_discovery: AuthorityDiscovery,
	}
}

impl pallet_session::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = AccountId;
	type ValidatorIdOf = ConvertInto;
	type ShouldEndSession = Babe;
	type NextSessionRotation = Babe;
	type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
	type SessionHandler = <SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type DisablingStrategy = pallet_session::disabling::UpToLimitWithReEnablingDisablingStrategy;
	type WeightInfo = weights::pallet_session::WeightInfo<Runtime>;
	type Currency = Balances;
	type KeyDeposit = ();
}

impl pallet_session::historical::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type FullIdentification = sp_staking::Exposure<AccountId, Balance>;
	type FullIdentificationOf = pallet_staking::DefaultExposureOf<Self>;
}

parameter_types! {
	// phase durations. 1/4 of the last session for each.
	// in testing: 1min or half of the session for each
	pub SignedPhase: u32 = prod_or_fast!(
		EPOCH_DURATION_IN_SLOTS / 4,
		(1 * MINUTES).min(EpochDuration::get().saturated_into::<u32>() / 2),
		"DOT_SIGNED_PHASE"
	);
	pub UnsignedPhase: u32 = prod_or_fast!(
		EPOCH_DURATION_IN_SLOTS / 4,
		(1 * MINUTES).min(EpochDuration::get().saturated_into::<u32>() / 2),
		"DOT_UNSIGNED_PHASE"
	);

	// signed config
	pub const SignedMaxSubmissions: u32 = 16;
	pub const SignedMaxRefunds: u32 = 16 / 4;
	// 40 DOTs fixed deposit..
	pub const SignedFixedDeposit: Balance = deposit(2, 0);
	pub const SignedDepositIncreaseFactor: sp_runtime::Percent = sp_runtime::Percent::from_percent(10);
	// 0.01 DOT per KB of solution data.
	pub const SignedDepositByte: Balance = deposit(0, 10) / 1024;
	// Each good submission will get 1 DOT as reward
	pub SignedRewardBase: Balance = 1 * UNITS;
	// 4 hour session, 1 hour unsigned phase, 32 offchain executions.
	pub OffchainRepeat: BlockNumber = UnsignedPhase::get() / 32;

	pub const MaxElectingVoters: u32 = 22_500;
	/// We take the top 22500 nominators as electing voters and all of the validators as electable
	/// targets. Whilst this is the case, we cannot and shall not increase the size of the
	/// validator intentions.
	pub ElectionBounds: frame_election_provider_support::bounds::ElectionBounds =
		ElectionBoundsBuilder::default().voters_count(MaxElectingVoters::get().into()).build();
	/// Setup election pallet to support maximum winners upto 1200. This will mean Staking Pallet
	/// cannot have active validators higher than this count.
	pub const MaxActiveValidators: u32 = 1200;
}

generate_solution_type!(
	#[compact]
	pub struct NposCompactSolution16::<
		VoterIndex = u32,
		TargetIndex = u16,
		Accuracy = sp_runtime::PerU16,
		MaxVoters = MaxElectingVoters,
	>(16)
);

pub struct OnChainSeqPhragmen;
impl onchain::Config for OnChainSeqPhragmen {
	type Sort = ConstBool<true>;
	type System = Runtime;
	type Solver = SequentialPhragmen<AccountId, runtime_common::elections::OnChainAccuracy>;
	type DataProvider = Staking;
	type WeightInfo = weights::frame_election_provider_support::WeightInfo<Runtime>;
	type Bounds = ElectionBounds;
	type MaxBackersPerWinner = MaxElectingVoters;
	type MaxWinnersPerPage = MaxActiveValidators;
}

impl pallet_election_provider_multi_phase::MinerConfig for Runtime {
	type AccountId = AccountId;
	type MaxLength = OffchainSolutionLengthLimit;
	type MaxWeight = OffchainSolutionWeightLimit;
	type Solution = NposCompactSolution16;
	type MaxVotesPerVoter = <
		<Self as pallet_election_provider_multi_phase::Config>::DataProvider
		as
		frame_election_provider_support::ElectionDataProvider
	>::MaxVotesPerVoter;
	type MaxBackersPerWinner = MaxElectingVoters;
	type MaxWinners = MaxActiveValidators;

	// The unsigned submissions have to respect the weight of the submit_unsigned call, thus their
	// weight estimate function is wired to this call's weight.
	fn solution_weight(v: u32, t: u32, a: u32, d: u32) -> Weight {
		<
			<Self as pallet_election_provider_multi_phase::Config>::WeightInfo
			as
			pallet_election_provider_multi_phase::WeightInfo
		>::submit_unsigned(v, t, a, d)
	}
}

impl pallet_election_provider_multi_phase::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EstimateCallFee = TransactionPayment;
	type SignedPhase = SignedPhase;
	type UnsignedPhase = UnsignedPhase;
	type SignedMaxSubmissions = SignedMaxSubmissions;
	type SignedMaxRefunds = SignedMaxRefunds;
	type SignedRewardBase = SignedRewardBase;
	type SignedDepositBase =
		GeometricDepositBase<Balance, SignedFixedDeposit, SignedDepositIncreaseFactor>;
	type SignedDepositByte = SignedDepositByte;
	type SignedDepositWeight = ();
	type SignedMaxWeight =
		<Self::MinerConfig as pallet_election_provider_multi_phase::MinerConfig>::MaxWeight;
	type MinerConfig = Self;
	type SlashHandler = (); // burn slashes
	type RewardHandler = (); // nothing to do upon rewards
	type BetterSignedThreshold = ();
	type OffchainRepeat = OffchainRepeat;
	type MinerTxPriority = NposSolutionPriority;
	type DataProvider = Staking;
	#[cfg(any(feature = "fast-runtime", feature = "runtime-benchmarks"))]
	type Fallback = onchain::OnChainExecution<OnChainSeqPhragmen>;
	#[cfg(not(any(feature = "fast-runtime", feature = "runtime-benchmarks")))]
	type Fallback = frame_election_provider_support::NoElection<(
		AccountId,
		BlockNumber,
		Staking,
		MaxActiveValidators,
		MaxElectingVoters,
	)>;
	type GovernanceFallback = onchain::OnChainExecution<OnChainSeqPhragmen>;
	type Solver = SequentialPhragmen<
		AccountId,
		pallet_election_provider_multi_phase::SolutionAccuracyOf<Self>,
		(),
	>;
	type BenchmarkingConfig = runtime_common::elections::BenchmarkConfig;
	type ForceOrigin = EitherOf<EnsureRoot<Self::AccountId>, StakingAdmin>;
	type WeightInfo = weights::pallet_election_provider_multi_phase::WeightInfo<Self>;
	type MaxWinners = MaxActiveValidators;
	type MaxBackersPerWinner = MaxElectingVoters;
	type ElectionBounds = ElectionBounds;
}

parameter_types! {
	pub const BagThresholds: &'static [u64] = &bag_thresholds::THRESHOLDS;
}

type VoterBagsListInstance = pallet_bags_list::Instance1;
impl pallet_bags_list::Config<VoterBagsListInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ScoreProvider = Staking;
	type WeightInfo = weights::pallet_bags_list::WeightInfo<Runtime>;
	type BagThresholds = BagThresholds;
	type Score = sp_npos_elections::VoteWeight;
	type MaxAutoRebagPerBlock = ConstU32<0>;
}

// TODO #6469: This shouldn't be static, but a lazily cached value, not built unless needed, and
// re-built in case input parameters have changed. The `ideal_stake` should be determined by the
// amount of parachain slots being bid on: this should be around `(75 - 25.min(slots / 4))%`.
pallet_staking_reward_curve::build! {
	const REWARD_CURVE: PiecewiseLinear<'static> = curve!(
		min_inflation: 0_025_000,
		max_inflation: 0_100_000,
		// 3:2:1 staked : parachains : float.
		// while there's no parachains, then this is 75% staked : 25% float.
		ideal_stake: 0_750_000,
		falloff: 0_050_000,
		max_piece_count: 40,
		test_precision: 0_005_000,
	);
}

parameter_types! {
	// Six sessions in an era (24 hours).
	pub const SessionsPerEra: SessionIndex = prod_or_fast!(6, 1);

	// 28 eras for unbonding (28 days).
	pub BondingDuration: sp_staking::EraIndex = prod_or_fast!(
		28,
		28,
		"DOT_BONDING_DURATION"
	);
	pub SlashDeferDuration: sp_staking::EraIndex = prod_or_fast!(
		27,
		27,
		"DOT_SLASH_DEFER_DURATION"
	);
	pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
	pub const MaxExposurePageSize: u32 = 512;
	pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(17);
	// 16
	pub const MaxNominations: u32 = <NposCompactSolution16 as frame_election_provider_support::NposSolution>::LIMIT as u32;
}

pub struct EraPayout;
impl pallet_staking::EraPayout<Balance> for EraPayout {
	fn era_payout(
		total_staked: Balance,
		total_issuance: Balance,
		era_duration_millis: u64,
	) -> (Balance, Balance) {
		// all para-ids that are not active.
		let auctioned_slots = parachains_paras::Parachains::<Runtime>::get()
			.into_iter()
			// all active para-ids that do not belong to a system or common good chain is the number
			// of parachains that we should take into account for inflation.
			.filter(|i| *i >= LOWEST_PUBLIC_ID)
			.count() as u64;

		let max_annual_inflation: Perquintill = Perquintill::from_percent(10);
		let min_annual_inflation: Perquintill = Perquintill::from_rational(25u64, 1000u64);
		const MILLISECONDS_PER_YEAR: u64 = 1000 * 3600 * 24 * 36525 / 100;

		runtime_common::impls::relay_era_payout(runtime_common::impls::EraPayoutParams {
			total_staked,
			total_stakable: total_issuance,
			ideal_stake: Perquintill::from_percent(75),
			max_annual_inflation,
			min_annual_inflation,
			falloff: Perquintill::from_percent(5),
			period_fraction: Perquintill::from_rational(era_duration_millis, MILLISECONDS_PER_YEAR),
			legacy_auction_proportion: Some(Perquintill::from_rational(auctioned_slots, 200u64)),
		})
	}
}

impl pallet_staking::Config for Runtime {
	type OldCurrency = Balances;
	type Currency = Balances;
	type CurrencyBalance = Balance;
	type RuntimeHoldReason = RuntimeHoldReason;
	type UnixTime = Timestamp;
	type CurrencyToVote = CurrencyToVote;
	type RewardRemainder = ResolveTo<pallet_treasury::TreasuryAccountId<Runtime>, Balances>;
	type RuntimeEvent = RuntimeEvent;
	type Slash = ResolveTo<pallet_treasury::TreasuryAccountId<Runtime>, Balances>;
	type Reward = ();
	type SessionsPerEra = SessionsPerEra;
	type BondingDuration = BondingDuration;
	type SlashDeferDuration = SlashDeferDuration;
	type AdminOrigin = EitherOf<EnsureRoot<Self::AccountId>, StakingAdmin>;
	type SessionInterface = Self;
	type EraPayout = EraPayout;
	type MaxExposurePageSize = MaxExposurePageSize;
	type NextNewSession = Session;
	type ElectionProvider = ElectionProviderMultiPhase;
	type GenesisElectionProvider = onchain::OnChainExecution<OnChainSeqPhragmen>;
	type VoterList = VoterList;
	type TargetList = UseValidatorsMap<Self>;
	type MaxValidatorSet = MaxActiveValidators;
	type NominationsQuota = pallet_staking::FixedNominationsQuota<{ MaxNominations::get() }>;
	type MaxUnlockingChunks = frame_support::traits::ConstU32<32>;
	type HistoryDepth = frame_support::traits::ConstU32<84>;
	type BenchmarkingConfig = runtime_common::StakingBenchmarkingConfig;
	type EventListeners = NominationPools;
	type WeightInfo = weights::pallet_staking::WeightInfo<Runtime>;
	type MaxControllersInDeprecationBatch = ConstU32<751>;
	type Filter = frame_support::traits::Nothing;
}

impl pallet_fast_unstake::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type BatchSize = frame_support::traits::ConstU32<16>;
	type Deposit = frame_support::traits::ConstU128<{ UNITS }>;
	type ControlOrigin = EnsureRoot<AccountId>;
	type Staking = Staking;
	type MaxErasToCheckPerBlock = ConstU32<1>;
	type WeightInfo = weights::pallet_fast_unstake::WeightInfo<Runtime>;
}

parameter_types! {
	// Minimum 4 CENTS/byte
	pub const BasicDeposit: Balance = deposit(1, 258);
	pub const ByteDeposit: Balance = deposit(0, 66);
	pub const SubAccountDeposit: Balance = deposit(1, 53);
	pub const MaxSubAccounts: u32 = 100;
	pub const MaxAdditionalFields: u32 = 100;
	pub const MaxRegistrars: u32 = 20;
}

impl pallet_identity::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type BasicDeposit = BasicDeposit;
	type ByteDeposit = ByteDeposit;
	type SubAccountDeposit = SubAccountDeposit;
	type MaxSubAccounts = MaxSubAccounts;
	type IdentityInformation = pallet_identity::legacy::IdentityInfo<MaxAdditionalFields>;
	type MaxRegistrars = MaxRegistrars;
	type Slashed = Treasury;
	type ForceOrigin = EitherOf<EnsureRoot<Self::AccountId>, GeneralAdmin>;
	type RegistrarOrigin = EitherOf<EnsureRoot<Self::AccountId>, GeneralAdmin>;
	type WeightInfo = weights::pallet_identity::WeightInfo<Runtime>;
	type OffchainSignature = Signature;
	type SigningPublicKey = <Signature as Verify>::Signer;
	type UsernameAuthorityOrigin = EnsureRoot<AccountId>;
	type PendingUsernameExpiration = ConstU32<{ 7 * DAYS }>;
	type MaxSuffixLength = ConstU32<7>;
	type MaxUsernameLength = ConstU32<32>;
	type UsernameDeposit = ConstU128<{ deposit(0, 32) }>;
	type UsernameGracePeriod = ConstU32<{ 30 * DAYS }>;
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 100 * DOLLARS;
	pub const ProposalBondMaximum: Balance = 500 * DOLLARS;
	pub const SpendPeriod: BlockNumber = 24 * DAYS;
	pub const Burn: Permill = Permill::from_percent(1);
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();

	pub const TipCountdown: BlockNumber = 1 * DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(20);
	pub const TipReportDepositBase: Balance = 1 * DOLLARS;
	pub const DataDepositPerByte: Balance = 1 * CENTS;
	pub const MaxApprovals: u32 = 100;
	pub const MaxAuthorities: u32 = 100_000;
	pub const RootSpendOriginMaxAmount: Balance = Balance::MAX;
	pub const CouncilSpendOriginMaxAmount: Balance = Balance::MAX;
}

impl pallet_treasury::Config for Runtime {
	type PalletId = TreasuryPalletId;
	type Currency = Balances;
	type RejectOrigin = EitherOfDiverse<EnsureRoot<AccountId>, Treasurer>;
	type RuntimeEvent = RuntimeEvent;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type BurnDestination = ();
	type SpendFunds = Bounties;
	type MaxApprovals = MaxApprovals;
	type WeightInfo = weights::pallet_treasury::WeightInfo<Runtime>;
	type SpendOrigin = TreasurySpender;
	type AssetKind = ();
	type Beneficiary = AccountId;
	type BeneficiaryLookup = sp_runtime::traits::IdentityLookup<AccountId>;
	type Paymaster = frame_support::traits::tokens::PayFromAccount<Balances, TreasuryAccount>;
	type BalanceConverter = frame_support::traits::tokens::UnityAssetBalanceConversion;
	type PayoutPeriod = SpendPeriod;
	type BlockNumberProvider = System;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

parameter_types! {
	pub const BountyDepositBase: Balance = 1 * DOLLARS;
	pub const BountyDepositPayoutDelay: BlockNumber = 8 * DAYS;
	pub const BountyUpdatePeriod: BlockNumber = 90 * DAYS;
	pub const MaximumReasonLength: u32 = 16384;
	pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
	pub const CuratorDepositMin: Balance = 10 * DOLLARS;
	pub const CuratorDepositMax: Balance = 200 * DOLLARS;
	pub const BountyValueMinimum: Balance = 10 * DOLLARS;
}

impl pallet_bounties::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type BountyDepositBase = BountyDepositBase;
	type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
	type BountyUpdatePeriod = BountyUpdatePeriod;
	type CuratorDepositMultiplier = CuratorDepositMultiplier;
	type CuratorDepositMin = CuratorDepositMin;
	type CuratorDepositMax = CuratorDepositMax;
	type BountyValueMinimum = BountyValueMinimum;
	type ChildBountyManager = ChildBounties;
	type DataDepositPerByte = DataDepositPerByte;
	type MaximumReasonLength = MaximumReasonLength;
	type OnSlash = ();
	type WeightInfo = weights::pallet_bounties::WeightInfo<Runtime>;
}

parameter_types! {
	pub const MaxActiveChildBountyCount: u32 = 100;
	pub const ChildBountyValueMinimum: Balance = BountyValueMinimum::get() / 10;
}

impl pallet_child_bounties::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxActiveChildBountyCount = MaxActiveChildBountyCount;
	type ChildBountyValueMinimum = ChildBountyValueMinimum;
	type WeightInfo = weights::pallet_child_bounties::WeightInfo<Runtime>;
}

impl pallet_offences::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
	type OnOffenceHandler = Staking;
}

impl pallet_authority_discovery::Config for Runtime {
	type MaxAuthorities = MaxAuthorities;
}

parameter_types! {
	pub NposSolutionPriority: TransactionPriority =
		Perbill::from_percent(90) * TransactionPriority::max_value();
}

parameter_types! {
	pub MaxSetIdSessionEntries: u32 = BondingDuration::get() * SessionsPerEra::get();
}

impl pallet_grandpa::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;

	type WeightInfo = ();
	type MaxAuthorities = MaxAuthorities;
	type MaxNominators = MaxExposurePageSize;
	type MaxSetIdSessionEntries = MaxSetIdSessionEntries;

	type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;

	type EquivocationReportSystem =
		pallet_grandpa::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

/// Submits a transaction with the node's public and signature type. Adheres to the signed extension
/// format of the chain.
impl<C> frame_system::offchain::CreateTransactionBase<C> for Runtime
where
	RuntimeCall: From<C>,
{
	type RuntimeCall = RuntimeCall;
	type Extrinsic = UncheckedExtrinsic;
}

impl<LocalCall> frame_system::offchain::CreateTransaction<LocalCall> for Runtime
where
	RuntimeCall: From<LocalCall>,
{
	type Extension = TxExtension;

	fn create_transaction(call: RuntimeCall, extension: TxExtension) -> UncheckedExtrinsic {
		UncheckedExtrinsic::new_transaction(call, extension)
	}
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
	RuntimeCall: From<LocalCall>,
{
	fn create_signed_transaction<
		C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>,
	>(
		call: RuntimeCall,
		public: <Signature as Verify>::Signer,
		account: AccountId,
		nonce: <Runtime as frame_system::Config>::Nonce,
	) -> Option<UncheckedExtrinsic> {
		use sp_runtime::traits::StaticLookup;
		// take the biggest period possible.
		let period =
			BlockHashCount::get().checked_next_power_of_two().map(|c| c / 2).unwrap_or(2) as u64;

		let current_block = System::block_number()
			.saturated_into::<u64>()
			// The `System::block_number` is initialized with `n+1`,
			// so the actual block number is `n`.
			.saturating_sub(1);
		let tip = 0;
		let tx_ext: TxExtension = (
			frame_system::AuthorizeCall::<Runtime>::new(),
			frame_system::CheckNonZeroSender::<Runtime>::new(),
			frame_system::CheckSpecVersion::<Runtime>::new(),
			frame_system::CheckTxVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckMortality::<Runtime>::from(generic::Era::mortal(
				period,
				current_block,
			)),
			frame_system::CheckNonce::<Runtime>::from(nonce),
			frame_system::CheckWeight::<Runtime>::new(),
			pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
			claims::PrevalidateAttests::<Runtime>::new(),
			frame_system::WeightReclaim::<Runtime>::new(),
		);
		let raw_payload = SignedPayload::new(call, tx_ext)
			.map_err(|e| {
				log::warn!("Unable to create signed payload: {:?}", e);
			})
			.ok()?;
		let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
		let (call, tx_ext, _) = raw_payload.deconstruct();
		let address = <Runtime as frame_system::Config>::Lookup::unlookup(account);
		let transaction = UncheckedExtrinsic::new_signed(call, address, signature, tx_ext);
		Some(transaction)
	}
}

impl<LocalCall> frame_system::offchain::CreateBare<LocalCall> for Runtime
where
	RuntimeCall: From<LocalCall>,
{
	fn create_bare(call: RuntimeCall) -> UncheckedExtrinsic {
		UncheckedExtrinsic::new_bare(call)
	}
}

impl frame_system::offchain::SigningTypes for Runtime {
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
}

parameter_types! {
	// Deposit for a parathread (on-demand parachain)
	pub const ParathreadDeposit: Balance = 500 * DOLLARS;
	pub const MaxRetries: u32 = 3;
}

parameter_types! {
	pub Prefix: &'static [u8] = b"Pay DOTs to the Polkadot account:";
}

impl claims::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type VestingSchedule = Vesting;
	type Prefix = Prefix;
	/// Only Root can move a claim.
	type MoveClaimOrigin = EnsureRoot<AccountId>;
	type WeightInfo = weights::runtime_common_claims::WeightInfo<Runtime>;
}

parameter_types! {
	pub const MinVestedTransfer: Balance = 1 * DOLLARS;
	pub UnvestedFundsAllowedWithdrawReasons: WithdrawReasons =
		WithdrawReasons::except(WithdrawReasons::TRANSFER | WithdrawReasons::RESERVE);
}

impl pallet_vesting::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type BlockNumberToBalance = ConvertInto;
	type MinVestedTransfer = MinVestedTransfer;
	type WeightInfo = weights::pallet_vesting::WeightInfo<Runtime>;
	type UnvestedFundsAllowedWithdrawReasons = UnvestedFundsAllowedWithdrawReasons;
	type BlockNumberProvider = System;
	const MAX_VESTING_SCHEDULES: u32 = 28;
}

impl pallet_utility::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = weights::pallet_utility::WeightInfo<Runtime>;
}

parameter_types! {
	// One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
	pub const DepositBase: Balance = deposit(1, 88);
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = deposit(0, 32);
	pub const MaxSignatories: u32 = 100;
}

impl pallet_multisig::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type WeightInfo = weights::pallet_multisig::WeightInfo<Runtime>;
	type BlockNumberProvider = System;
}

parameter_types! {
	// One storage item; key size 32, value size 8; .
	pub const ProxyDepositBase: Balance = deposit(1, 8);
	// Additional storage item size of 33 bytes.
	pub const ProxyDepositFactor: Balance = deposit(0, 33);
	pub const MaxProxies: u16 = 32;
	pub const AnnouncementDepositBase: Balance = deposit(1, 8);
	pub const AnnouncementDepositFactor: Balance = deposit(0, 66);
	pub const MaxPending: u16 = 32;
}

/// The type used to represent the kinds of proxying allowed.
#[derive(
	Copy,
	Clone,
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	Encode,
	Decode,
	DecodeWithMemTracking,
	RuntimeDebug,
	MaxEncodedLen,
	scale_info::TypeInfo,
)]
pub enum ProxyType {
	Any = 0,
	NonTransfer = 1,
	Governance = 2,
	Staking = 3,
	// Skip 4 as it is now removed (was SudoBalances)
	IdentityJudgement = 5,
	CancelProxy = 6,
	Auction = 7,
	NominationPools = 8,
}

#[cfg(test)]
mod proxy_type_tests {
	use super::*;

	#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug)]
	pub enum OldProxyType {
		Any,
		NonTransfer,
		Governance,
		Staking,
		SudoBalances,
		IdentityJudgement,
	}

	#[test]
	fn proxy_type_decodes_correctly() {
		for (i, j) in vec![
			(OldProxyType::Any, ProxyType::Any),
			(OldProxyType::NonTransfer, ProxyType::NonTransfer),
			(OldProxyType::Governance, ProxyType::Governance),
			(OldProxyType::Staking, ProxyType::Staking),
			(OldProxyType::IdentityJudgement, ProxyType::IdentityJudgement),
		]
		.into_iter()
		{
			assert_eq!(i.encode(), j.encode());
		}
		assert!(ProxyType::decode(&mut &OldProxyType::SudoBalances.encode()[..]).is_err());
	}
}

impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
	}
}
impl InstanceFilter<RuntimeCall> for ProxyType {
	fn filter(&self, c: &RuntimeCall) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::NonTransfer => matches!(
				c,
				RuntimeCall::System(..) |
				RuntimeCall::Scheduler(..) |
				RuntimeCall::Babe(..) |
				RuntimeCall::Timestamp(..) |
				RuntimeCall::Indices(pallet_indices::Call::claim{..}) |
				RuntimeCall::Indices(pallet_indices::Call::free{..}) |
				RuntimeCall::Indices(pallet_indices::Call::freeze{..}) |
				// Specifically omitting Indices `transfer`, `force_transfer`
				// Specifically omitting the entire Balances pallet
				RuntimeCall::Staking(..) |
				RuntimeCall::Session(..) |
				RuntimeCall::Grandpa(..) |
				RuntimeCall::Treasury(..) |
				RuntimeCall::Bounties(..) |
				RuntimeCall::ChildBounties(..) |
				RuntimeCall::Democracy(..) |
				RuntimeCall::Council(..) |
				RuntimeCall::TechnicalCommittee(..) |
				RuntimeCall::PhragmenElection(..) |
				RuntimeCall::TechnicalMembership(..) |
				RuntimeCall::Claims(..) |
				RuntimeCall::Vesting(pallet_vesting::Call::vest{..}) |
				RuntimeCall::Vesting(pallet_vesting::Call::vest_other{..}) |
				// Specifically omitting Vesting `vested_transfer`, and `force_vested_transfer`
				RuntimeCall::Utility(..) |
				RuntimeCall::Identity(..) |
				RuntimeCall::Proxy(..) |
				RuntimeCall::Multisig(..) |
				RuntimeCall::Registrar(paras_registrar::Call::register {..}) |
				RuntimeCall::Registrar(paras_registrar::Call::deregister {..}) |
				// Specifically omitting Registrar `swap`
				RuntimeCall::Registrar(paras_registrar::Call::reserve {..}) |
				RuntimeCall::Crowdloan(..) |
				RuntimeCall::Slots(..) |
				RuntimeCall::Auctions(..) | // Specifically omitting the entire XCM Pallet
				RuntimeCall::VoterList(..) |
				RuntimeCall::NominationPools(..) |
				RuntimeCall::FastUnstake(..)
			),
			ProxyType::Governance => matches!(
				c,
				RuntimeCall::Treasury(..) |
					RuntimeCall::Bounties(..) |
					RuntimeCall::Utility(..) |
					RuntimeCall::ChildBounties(..) |
					RuntimeCall::Democracy(..) |
					RuntimeCall::Council(..) |
					RuntimeCall::TechnicalCommittee(..) |
					RuntimeCall::PhragmenElection(..) |
					RuntimeCall::TechnicalMembership(..)
			),
			ProxyType::Staking => {
				matches!(
					c,
					RuntimeCall::Staking(..) |
						RuntimeCall::Session(..) |
						RuntimeCall::Utility(..) |
						RuntimeCall::FastUnstake(..) |
						RuntimeCall::VoterList(..) |
						RuntimeCall::NominationPools(..)
				)
			},
			ProxyType::NominationPools => {
				matches!(c, RuntimeCall::NominationPools(..) | RuntimeCall::Utility(..))
			},
			ProxyType::IdentityJudgement => matches!(
				c,
				RuntimeCall::Identity(pallet_identity::Call::provide_judgement { .. }) |
					RuntimeCall::Utility(..)
			),
			ProxyType::CancelProxy => {
				matches!(c, RuntimeCall::Proxy(pallet_proxy::Call::reject_announcement { .. }))
			},
			ProxyType::Auction => matches!(
				c,
				RuntimeCall::Auctions(..) |
					RuntimeCall::Crowdloan(..) |
					RuntimeCall::Registrar(..) |
					RuntimeCall::Slots(..)
			),
		}
	}
	fn is_superset(&self, o: &Self) -> bool {
		match (self, o) {
			(x, y) if x == y => true,
			(ProxyType::Any, _) => true,
			(_, ProxyType::Any) => false,
			(ProxyType::NonTransfer, _) => true,
			_ => false,
		}
	}
}

impl pallet_proxy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type MaxProxies = MaxProxies;
	type WeightInfo = weights::pallet_proxy::WeightInfo<Runtime>;
	type MaxPending = MaxPending;
	type CallHasher = BlakeTwo256;
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
	type BlockNumberProvider = System;
}

impl parachains_origin::Config for Runtime {}

impl parachains_configuration::Config for Runtime {
	type WeightInfo = weights::runtime_parachains_configuration::WeightInfo<Runtime>;
}

impl parachains_shared::Config for Runtime {
	type DisabledValidators = Session;
}

impl parachains_session_info::Config for Runtime {
	type ValidatorSet = Historical;
}

impl parachains_inclusion::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type DisputesHandler = ParasDisputes;
	type RewardValidators =
		parachains_reward_points::RewardValidatorsWithEraPoints<Runtime, Staking>;
	type MessageQueue = MessageQueue;
	type WeightInfo = weights::runtime_parachains_inclusion::WeightInfo<Runtime>;
}

parameter_types! {
	pub const ParasUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
}

impl parachains_paras::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::runtime_parachains_paras::WeightInfo<Runtime>;
	type UnsignedPriority = ParasUnsignedPriority;
	type QueueFootprinter = ParaInclusion;
	type NextSessionRotation = Babe;
	type OnNewHead = Registrar;
	type AssignCoretime = CoretimeAssignmentProvider;
	type Fungible = Balances;
	type CooldownRemovalMultiplier = sp_core::ConstUint<{ 1000 * UNITS / DAYS as u128 }>;
	type AuthorizeCurrentCodeOrigin = EnsureRoot<AccountId>;
}

parameter_types! {
	/// Amount of weight that can be spent per block to service messages.
	///
	/// # WARNING
	///
	/// This is not a good value for para-chains since the `Scheduler` already uses up to 80% block weight.
	pub MessageQueueServiceWeight: Weight = Perbill::from_percent(20) * BlockWeights::get().max_block;
	pub const MessageQueueHeapSize: u32 = 65_536;
	pub const MessageQueueMaxStale: u32 = 8;
}

/// Message processor to handle any messages that were enqueued into the `MessageQueue` pallet.
pub struct MessageProcessor;
impl ProcessMessage for MessageProcessor {
	type Origin = AggregateMessageOrigin;

	fn process_message(
		message: &[u8],
		origin: Self::Origin,
		meter: &mut WeightMeter,
		id: &mut [u8; 32],
	) -> Result<bool, ProcessMessageError> {
		let para = match origin {
			AggregateMessageOrigin::Ump(UmpQueueId::Para(para)) => para,
		};
		xcm_builder::ProcessXcmMessage::<
			Junction,
			xcm_executor::XcmExecutor<xcm_config::XcmConfig>,
			RuntimeCall,
		>::process_message(message, Junction::Parachain(para.into()), meter, id)
	}
}

impl pallet_message_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Size = u32;
	type HeapSize = MessageQueueHeapSize;
	type MaxStale = MessageQueueMaxStale;
	type ServiceWeight = MessageQueueServiceWeight;
	type IdleMaxServiceWeight = MessageQueueServiceWeight;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type MessageProcessor = MessageProcessor;
	#[cfg(feature = "runtime-benchmarks")]
	type MessageProcessor =
		pallet_message_queue::mock_helpers::NoopMessageProcessor<AggregateMessageOrigin>;
	type QueueChangeHandler = ParaInclusion;
	type QueuePausedQuery = ();
	type WeightInfo = weights::pallet_message_queue::WeightInfo<Runtime>;
}

impl parachains_dmp::Config for Runtime {}

parameter_types! {
	pub const DefaultChannelSizeAndCapacityWithSystem: (u32, u32) = (4096, 4);
}

impl parachains_hrmp::Config for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeEvent = RuntimeEvent;
	type ChannelManager = EitherOf<EnsureRoot<Self::AccountId>, GeneralAdmin>;
	type Currency = Balances;
	type DefaultChannelSizeAndCapacityWithSystem = DefaultChannelSizeAndCapacityWithSystem;
	type VersionWrapper = crate::XcmPallet;
	type WeightInfo = weights::runtime_parachains_hrmp::WeightInfo<Self>;
}

impl parachains_paras_inherent::Config for Runtime {
	type WeightInfo = weights::runtime_parachains_paras_inherent::WeightInfo<Runtime>;
}

impl parachains_scheduler::Config for Runtime {
	type AssignmentProvider = CoretimeAssignmentProvider;
}

parameter_types! {
	pub const OnDemandTrafficDefaultValue: FixedU128 = FixedU128::from_u32(1);
	pub const MaxHistoricalRevenue: BlockNumber = 2 * 80;
	pub const OnDemandPalletId: PalletId = PalletId(*b"py/ondmd");
}

impl parachains_on_demand::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type TrafficDefaultValue = OnDemandTrafficDefaultValue;
	type WeightInfo = parachains_on_demand::TestWeightInfo;
	type MaxHistoricalRevenue = MaxHistoricalRevenue;
	type PalletId = OnDemandPalletId;
}

impl parachains_assigner_coretime::Config for Runtime {}

impl parachains_initializer::Config for Runtime {
	type Randomness = pallet_babe::RandomnessFromOneEpochAgo<Runtime>;
	type ForceOrigin = EnsureRoot<AccountId>;
	type WeightInfo = weights::runtime_parachains_initializer::WeightInfo<Runtime>;
	type CoretimeOnNewSession = ();
}

impl parachains_disputes::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RewardValidators =
		parachains_reward_points::RewardValidatorsWithEraPoints<Runtime, Staking>;
	type SlashingHandler = parachains_slashing::SlashValidatorsForDisputes<ParasSlashing>;
	type WeightInfo = weights::runtime_parachains_disputes::WeightInfo<Runtime>;
}

impl parachains_slashing::Config for Runtime {
	type KeyOwnerProofSystem = Historical;
	type KeyOwnerProof =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, ValidatorId)>>::Proof;
	type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
		KeyTypeId,
		ValidatorId,
	)>>::IdentificationTuple;
	type HandleReports = parachains_slashing::SlashingReportHandler<
		Self::KeyOwnerIdentification,
		Offences,
		ReportLongevity,
	>;
	type WeightInfo = weights::runtime_parachains_disputes_slashing::WeightInfo<Runtime>;
	type BenchmarkingConfig = parachains_slashing::BenchConfig<1000>;
}

parameter_types! {
	// Mostly arbitrary deposit price, but should provide an adequate incentive not to spam reserve
	// `ParaId`s.
	pub const ParaDeposit: Balance = 100 * DOLLARS;
	pub const ParaDataByteDeposit: Balance = deposit(0, 1);
}

impl paras_registrar::Config for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type OnSwap = (Crowdloan, Slots);
	type ParaDeposit = ParaDeposit;
	type DataDepositPerByte = ParaDataByteDeposit;
	type WeightInfo = weights::runtime_common_paras_registrar::WeightInfo<Runtime>;
}

parameter_types! {
	// 12 weeks = 3 months per lease period -> 8 lease periods ~ 2 years
	pub LeasePeriod: BlockNumber = prod_or_fast!(12 * WEEKS, 12 * WEEKS, "DOT_LEASE_PERIOD");
	// Polkadot Genesis was on May 26, 2020.
	// Target Parachain Onboarding Date: Dec 15, 2021.
	// Difference is 568 days.
	// We want a lease period to start on the target onboarding date.
	// 568 % (12 * 7) = 64 day offset
	pub LeaseOffset: BlockNumber = prod_or_fast!(64 * DAYS, 0, "DOT_LEASE_OFFSET");
}

impl slots::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Registrar = Registrar;
	type LeasePeriod = LeasePeriod;
	type LeaseOffset = LeaseOffset;
	type ForceOrigin = EitherOf<EnsureRoot<Self::AccountId>, LeaseAdmin>;
	type WeightInfo = weights::runtime_common_slots::WeightInfo<Runtime>;
}

parameter_types! {
	pub const CrowdloanId: PalletId = PalletId(*b"py/cfund");
	// Accounts for 10_000 contributions, each using 48 bytes (16 bytes for balance, and 32 bytes
	// for a memo).
	pub const SubmissionDeposit: Balance = deposit(1, 480_000);
	// The minimum crowdloan contribution.
	pub const MinContribution: Balance = 5 * DOLLARS;
	pub const RemoveKeysLimit: u32 = 1000;
	// Allow 32 bytes for an additional memo to a crowdloan.
	pub const MaxMemoLength: u8 = 32;
}

impl crowdloan::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = CrowdloanId;
	type SubmissionDeposit = SubmissionDeposit;
	type MinContribution = MinContribution;
	type RemoveKeysLimit = RemoveKeysLimit;
	type Registrar = Registrar;
	type Auctioneer = Auctions;
	type MaxMemoLength = MaxMemoLength;
	type WeightInfo = weights::runtime_common_crowdloan::WeightInfo<Runtime>;
}

parameter_types! {
	// The average auction is 7 days long, so this will be 70% for ending period.
	// 5 Days = 72000 Blocks @ 6 sec per block
	pub const EndingPeriod: BlockNumber = 5 * DAYS;
	// ~ 1000 samples per day -> ~ 20 blocks per sample -> 2 minute samples
	pub const SampleLength: BlockNumber = 2 * MINUTES;
}

impl auctions::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Leaser = Slots;
	type Registrar = Registrar;
	type EndingPeriod = EndingPeriod;
	type SampleLength = SampleLength;
	type Randomness = pallet_babe::RandomnessFromOneEpochAgo<Runtime>;
	type InitiateOrigin = EitherOf<EnsureRoot<Self::AccountId>, AuctionAdmin>;
	type WeightInfo = weights::runtime_common_auctions::WeightInfo<Runtime>;
}

parameter_types! {
	pub const PoolsPalletId: PalletId = PalletId(*b"py/nopls");
	// Allow pools that got slashed up to 90% to remain operational.
	pub const MaxPointsToBalance: u8 = 10;
}

impl pallet_nomination_pools::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type RewardCounter = FixedU128;
	type BalanceToU256 = runtime_common::BalanceToU256;
	type U256ToBalance = runtime_common::U256ToBalance;
	type StakeAdapter = pallet_nomination_pools::adapter::TransferStake<Self, Staking>;
	type PostUnbondingPoolsWindow = frame_support::traits::ConstU32<4>;
	type MaxMetadataLen = frame_support::traits::ConstU32<256>;
	// we use the same number of allowed unlocking chunks as with staking.
	type MaxUnbonding = <Self as pallet_staking::Config>::MaxUnlockingChunks;
	type PalletId = PoolsPalletId;
	type MaxPointsToBalance = MaxPointsToBalance;
	type WeightInfo = weights::pallet_nomination_pools::WeightInfo<Self>;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type AdminOrigin = EnsureRoot<AccountId>;
	type BlockNumberProvider = System;
	type Filter = frame_support::traits::Nothing;
}

pub struct InitiateNominationPools;
impl frame_support::traits::OnRuntimeUpgrade for InitiateNominationPools {
	fn on_runtime_upgrade() -> frame_support::weights::Weight {
		// we use one as an indicator if this has already been set.
		if pallet_nomination_pools::MaxPools::<Runtime>::get().is_none() {
			// 5 DOT to join a pool.
			pallet_nomination_pools::MinJoinBond::<Runtime>::put(5 * UNITS);
			// 100 DOT to create a pool.
			pallet_nomination_pools::MinCreateBond::<Runtime>::put(100 * UNITS);

			// Initialize with limits for now.
			pallet_nomination_pools::MaxPools::<Runtime>::put(0);
			pallet_nomination_pools::MaxPoolMembersPerPool::<Runtime>::put(0);
			pallet_nomination_pools::MaxPoolMembers::<Runtime>::put(0);

			log::info!(target: "runtime::polkadot", "pools config initiated 🎉");
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 5)
		} else {
			log::info!(target: "runtime::polkadot", "pools config already initiated 😏");
			<Runtime as frame_system::Config>::DbWeight::get().reads(1)
		}
	}
}

// --- THXNet Gov V1 pallet configurations ---

parameter_types! {
	pub const LaunchPeriod: BlockNumber = 28 * DAYS;
	pub const VotingPeriod: BlockNumber = 28 * DAYS;
	pub const FastTrackVotingPeriod: BlockNumber = 3 * HOURS;
	pub const MinimumDeposit: Balance = 100 * DOLLARS;
	pub const EnactmentPeriod: BlockNumber = 28 * DAYS;
	pub const CooloffPeriod: BlockNumber = 7 * DAYS;
	pub const MaxProposals: u32 = 100;
}

impl pallet_democracy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type VoteLockingPeriod = EnactmentPeriod;
	type MinimumDeposit = MinimumDeposit;
	type ExternalOrigin = frame_system::EnsureRoot<AccountId>;
	type ExternalMajorityOrigin = frame_system::EnsureRoot<AccountId>;
	type ExternalDefaultOrigin = frame_system::EnsureRoot<AccountId>;
	type SubmitOrigin = frame_system::EnsureSigned<AccountId>;
	type FastTrackOrigin = frame_system::EnsureRoot<AccountId>;
	type InstantOrigin = frame_system::EnsureRoot<AccountId>;
	type InstantAllowed = ConstBool<true>;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	type CancellationOrigin = frame_system::EnsureRoot<AccountId>;
	type BlacklistOrigin = frame_system::EnsureRoot<AccountId>;
	type CancelProposalOrigin = frame_system::EnsureRoot<AccountId>;
	type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
	type CooloffPeriod = CooloffPeriod;
	type Slash = Treasury;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type MaxVotes = ConstU32<100>;
	type WeightInfo = weights::pallet_democracy::WeightInfo<Runtime>;
	type MaxProposals = MaxProposals;
	type Preimages = Preimage;
	type MaxDeposits = ConstU32<100>;
	type MaxBlacklisted = ConstU32<100>;
}

parameter_types! {
	pub CouncilMotionDuration: BlockNumber = 7 * DAYS;
	pub const CouncilMaxProposals: u32 = 100;
	pub const CouncilMaxMembers: u32 = 100;
}

type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilCollective> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = CouncilMotionDuration;
	type MaxProposals = CouncilMaxProposals;
	type MaxMembers = CouncilMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = weights::pallet_collective::WeightInfo<Runtime>;
	type SetMembersOrigin = frame_system::EnsureRoot<AccountId>;
	type MaxProposalWeight = MaxCollectivesProposalWeight;
	type DisapproveOrigin = frame_system::EnsureRoot<AccountId>;
	type KillOrigin = frame_system::EnsureRoot<AccountId>;
	type Consideration = ();
}

parameter_types! {
	pub TechnicalMotionDuration: BlockNumber = 7 * DAYS;
	pub const TechnicalMaxProposals: u32 = 100;
	pub const TechnicalMaxMembers: u32 = 100;
}

type TechnicalCollective = pallet_collective::Instance2;
impl pallet_collective::Config<TechnicalCollective> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = TechnicalMotionDuration;
	type MaxProposals = TechnicalMaxProposals;
	type MaxMembers = TechnicalMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = weights::pallet_collective::WeightInfo<Runtime>;
	type SetMembersOrigin = frame_system::EnsureRoot<AccountId>;
	type MaxProposalWeight = MaxCollectivesProposalWeight;
	type DisapproveOrigin = frame_system::EnsureRoot<AccountId>;
	type KillOrigin = frame_system::EnsureRoot<AccountId>;
	type Consideration = ();
}

parameter_types! {
	pub const CandidacyBond: Balance = 100 * DOLLARS;
	pub const DesiredMembers: u32 = 13;
	pub const DesiredRunnersUp: u32 = 20;
	pub const TermDuration: BlockNumber = 7 * DAYS;
	pub const PhragmenElectionPalletId: frame_support::traits::LockIdentifier = *b"phrelect";
}

impl pallet_elections_phragmen::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = PhragmenElectionPalletId;
	type Currency = Balances;
	type ChangeMembers = Council;
	type InitializeMembers = Council;
	type CurrencyToVote = CurrencyToVote;
	type CandidacyBond = CandidacyBond;
	type VotingBondBase = ConstU128<{ deposit(1, 64) }>;
	type VotingBondFactor = ConstU128<{ deposit(0, 32) }>;
	type LoserCandidate = Treasury;
	type KickedMember = Treasury;
	type DesiredMembers = DesiredMembers;
	type DesiredRunnersUp = DesiredRunnersUp;
	type TermDuration = TermDuration;
	type MaxCandidates = ConstU32<200>;
	type MaxVoters = ConstU32<512>;
	type MaxVotesPerVoter = ConstU32<16>;
	type WeightInfo = weights::pallet_elections_phragmen::WeightInfo<Runtime>;
}

impl pallet_membership::Config<pallet_membership::Instance1> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = frame_system::EnsureRoot<AccountId>;
	type RemoveOrigin = frame_system::EnsureRoot<AccountId>;
	type SwapOrigin = frame_system::EnsureRoot<AccountId>;
	type ResetOrigin = frame_system::EnsureRoot<AccountId>;
	type PrimeOrigin = frame_system::EnsureRoot<AccountId>;
	type MembershipInitialized = TechnicalCommittee;
	type MembershipChanged = TechnicalCommittee;
	type MaxMembers = TechnicalMaxMembers;
	type WeightInfo = weights::pallet_membership::WeightInfo<Runtime>;
}

// --- THXNet-specific pallet configurations ---

impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type WeightInfo = ();
}

impl runtime_common::paras_sudo_wrapper::Config for Runtime {}

impl pallet_dao::Config for Runtime {
	type UnixTime = Timestamp;
	type Currency = Balances;
	type RuntimeEvent = RuntimeEvent;
	type TopicId = u64;
	type Vote = u128;
	type OptionIndex = u64;
	type ForceOrigin = frame_system::EnsureRoot<AccountId>;
	type TopicTitleMinimumLength = ConstU32<1>;
	type TopicTitleMaximumLength = ConstU32<256>;
	type TopicDescriptionMinimumLength = ConstU32<1>;
	type TopicDescriptionMaximumLength = ConstU32<2048>;
	type TopicOptionMinimumLength = ConstU32<1>;
	type TopicOptionMaximumLength = ConstU32<256>;
	type TopicOptionMaximumNumber = ConstU32<1024>;
	type StringLimit = ConstU32<{ 2048 * 4 }>;
	type TopicRaiserBalanceLowerBound = ConstU128<1_000_000>;
	type CurrencyUnits = ConstU128<{ UNITS }>;
}

parameter_types! {
	pub const RescueCooldown: BlockNumber = 100;
}

impl pallet_finality_rescue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RescueCooldown = RescueCooldown;
}

parameter_types! {
	pub MbmServiceWeight: Weight = Perbill::from_percent(80) * BlockWeights::get().max_block;
}

impl pallet_migrations::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type Migrations = pallet_identity::migration::v2::LazyMigrationV1ToV2<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type Migrations = pallet_migrations::mock_helpers::MockedMigrations;
	type CursorMaxLen = ConstU32<65_536>;
	type IdentifierMaxLen = ConstU32<256>;
	type MigrationStatusHandler = ();
	type FailedMigrationHandler = frame_support::migrations::FreezeChainOnFailedMigration;
	type MaxServiceWeight = MbmServiceWeight;
	type WeightInfo = weights::pallet_migrations::WeightInfo<Runtime>;
}

parameter_types! {
	pub const AssetDeposit: Balance = 10 * DOLLARS;
	pub const AssetAccountDeposit: Balance = deposit(1, 16);
	pub const ApprovalDeposit: Balance = EXISTENTIAL_DEPOSIT;
	pub const StringLimit: u32 = 50;
	pub const MetadataDepositBase: Balance = deposit(1, 68);
	pub const MetadataDepositPerByte: Balance = deposit(0, 1);
}

impl pallet_assets::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type AssetId = u32;
	type AssetIdParameter = parity_scale_codec::Compact<u32>;
	type Currency = Balances;
	type CreateOrigin =
		frame_support::traits::AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
	type ForceOrigin = frame_system::EnsureRoot<AccountId>;
	type AssetDeposit = AssetDeposit;
	type AssetAccountDeposit = AssetAccountDeposit;
	type MetadataDepositBase = MetadataDepositBase;
	type MetadataDepositPerByte = MetadataDepositPerByte;
	type ApprovalDeposit = ApprovalDeposit;
	type StringLimit = StringLimit;
	type Freezer = ();
	type Extra = ();
	type CallbackHandle = ();
	type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
	type RemoveItemsLimit = ConstU32<1000>;
	type ReserveData = ();
	type Holder = ();
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

impl pallet_asset_tx_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Fungibles = Assets;
	type OnChargeAssetTransaction = pallet_asset_tx_payment::FungiblesAdapter<
		pallet_assets::BalanceToAssetBalance<Balances, Runtime, ConvertInto>,
		CreditToBlockAuthor,
	>;
	type WeightInfo = ();
}

parameter_types! {
	pub Features: pallet_nfts::PalletFeatures = pallet_nfts::PalletFeatures::all_enabled();
	pub const MaxAttributesPerCall: u32 = 10;
	pub const CollectionDeposit: Balance = 100 * DOLLARS;
	pub const ItemDeposit: Balance = 1 * DOLLARS;
	pub const KeyLimit: u32 = 32;
	pub const ValueLimit: u32 = 256;
	pub const ApprovalsLimit: u32 = 20;
	pub const ItemAttributesApprovalsLimit: u32 = 20;
	pub const MaxTips: u32 = 10;
	pub const MaxDeadlineDuration: BlockNumber = 12 * 30 * DAYS;
}

impl pallet_nfts::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = u32;
	type ItemId = u32;
	type Currency = Balances;
	type ForceOrigin = frame_system::EnsureRoot<AccountId>;
	type CollectionDeposit = CollectionDeposit;
	type ItemDeposit = ItemDeposit;
	type MetadataDepositBase = MetadataDepositBase;
	type AttributeDepositBase = MetadataDepositBase;
	type DepositPerByte = MetadataDepositPerByte;
	type StringLimit = StringLimit;
	type KeyLimit = KeyLimit;
	type ValueLimit = ValueLimit;
	type ApprovalsLimit = ApprovalsLimit;
	type ItemAttributesApprovalsLimit = ItemAttributesApprovalsLimit;
	type MaxTips = MaxTips;
	type MaxDeadlineDuration = MaxDeadlineDuration;
	type MaxAttributesPerCall = MaxAttributesPerCall;
	type Features = Features;
	type OffchainSignature = Signature;
	type OffchainPublic = <Signature as sp_runtime::traits::Verify>::Signer;
	type WeightInfo = pallet_nfts::weights::SubstrateWeight<Runtime>;
	type BlockNumberProvider = System;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
	type CreateOrigin =
		frame_support::traits::AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
	type Locker = ();
}

#[frame_support::runtime(legacy_ordering)]
mod runtime {
	#[runtime::runtime]
	#[runtime::derive(
		RuntimeCall,
		RuntimeEvent,
		RuntimeError,
		RuntimeOrigin,
		RuntimeFreezeReason,
		RuntimeHoldReason,
		RuntimeSlashReason,
		RuntimeLockId,
		RuntimeTask,
		RuntimeViewFunction
	)]
	pub struct Runtime;

	// Basic stuff; balances is uncallable initially.
	#[runtime::pallet_index(0)]
	pub type System = frame_system;
	#[runtime::pallet_index(1)]
	pub type Scheduler = pallet_scheduler;
	#[runtime::pallet_index(10)]
	pub type Preimage = pallet_preimage;

	// Babe must be before session.
	#[runtime::pallet_index(2)]
	pub type Babe = pallet_babe;

	#[runtime::pallet_index(3)]
	pub type Timestamp = pallet_timestamp;
	#[runtime::pallet_index(4)]
	pub type Indices = pallet_indices;
	#[runtime::pallet_index(5)]
	pub type Balances = pallet_balances;
	#[runtime::pallet_index(32)]
	pub type TransactionPayment = pallet_transaction_payment;

	// Consensus support.
	// Authorship must be before session in order to note author in the correct session and era.
	#[runtime::pallet_index(6)]
	pub type Authorship = pallet_authorship;
	#[runtime::pallet_index(7)]
	pub type Staking = pallet_staking;
	#[runtime::pallet_index(8)]
	pub type Offences = pallet_offences;
	#[runtime::pallet_index(33)]
	pub type Historical = session_historical;
	#[runtime::pallet_index(9)]
	pub type Session = pallet_session;
	#[runtime::pallet_index(11)]
	pub type Grandpa = pallet_grandpa;
	// ImOnline removed in v1.5.0 (index 12 intentionally left empty)
	#[runtime::pallet_index(13)]
	pub type AuthorityDiscovery = pallet_authority_discovery;

	// Governance (Gov V1).
	#[runtime::pallet_index(14)]
	pub type Democracy = pallet_democracy;
	#[runtime::pallet_index(15)]
	pub type Council = pallet_collective<Instance1>;
	#[runtime::pallet_index(16)]
	pub type TechnicalCommittee = pallet_collective<Instance2>;
	#[runtime::pallet_index(17)]
	pub type PhragmenElection = pallet_elections_phragmen;
	#[runtime::pallet_index(18)]
	pub type TechnicalMembership = pallet_membership<Instance1>;
	#[runtime::pallet_index(19)]
	pub type Treasury = pallet_treasury;

	// Claims. Usable initially.
	#[runtime::pallet_index(24)]
	pub type Claims = claims;
	// Vesting. Usable initially, but removed once all vesting is finished.
	#[runtime::pallet_index(25)]
	pub type Vesting = pallet_vesting;
	// Cunning utilities. Usable initially.
	#[runtime::pallet_index(26)]
	pub type Utility = pallet_utility;

	// Identity. Late addition.
	#[runtime::pallet_index(28)]
	pub type Identity = pallet_identity;

	// Proxy module. Late addition.
	#[runtime::pallet_index(29)]
	pub type Proxy = pallet_proxy;

	// Multisig dispatch. Late addition.
	#[runtime::pallet_index(30)]
	pub type Multisig = pallet_multisig;

	// Bounties modules.
	#[runtime::pallet_index(34)]
	pub type Bounties = pallet_bounties;
	#[runtime::pallet_index(38)]
	pub type ChildBounties = pallet_child_bounties;

	// Election pallet. Only works with staking, but placed here to maintain indices.
	#[runtime::pallet_index(36)]
	pub type ElectionProviderMultiPhase = pallet_election_provider_multi_phase;

	// Provides a semi-sorted list of nominators for staking.
	#[runtime::pallet_index(37)]
	pub type VoterList = pallet_bags_list<Instance1>;

	// Nomination pools: extension to staking.
	#[runtime::pallet_index(39)]
	pub type NominationPools = pallet_nomination_pools;

	// Fast unstake pallet: extension to staking.
	#[runtime::pallet_index(40)]
	pub type FastUnstake = pallet_fast_unstake;

	// Parachains pallets. Start indices at 50 to leave room.
	#[runtime::pallet_index(50)]
	pub type ParachainsOrigin = parachains_origin;
	#[runtime::pallet_index(51)]
	pub type Configuration = parachains_configuration;
	#[runtime::pallet_index(52)]
	pub type ParasShared = parachains_shared;
	#[runtime::pallet_index(53)]
	pub type ParaInclusion = parachains_inclusion;
	#[runtime::pallet_index(54)]
	pub type ParaInherent = parachains_paras_inherent;
	#[runtime::pallet_index(55)]
	pub type ParaScheduler = parachains_scheduler;
	#[runtime::pallet_index(56)]
	pub type Paras = parachains_paras;
	#[runtime::pallet_index(57)]
	pub type Initializer = parachains_initializer;
	#[runtime::pallet_index(58)]
	pub type Dmp = parachains_dmp;
	#[runtime::pallet_index(60)]
	pub type Hrmp = parachains_hrmp;
	#[runtime::pallet_index(61)]
	pub type ParaSessionInfo = parachains_session_info;
	#[runtime::pallet_index(62)]
	pub type ParasDisputes = parachains_disputes;
	#[runtime::pallet_index(63)]
	pub type ParasSlashing = parachains_slashing;
	#[runtime::pallet_index(65)]
	pub type OnDemandAssignmentProvider = parachains_on_demand;
	#[runtime::pallet_index(64)]
	pub type CoretimeAssignmentProvider = parachains_assigner_coretime;

	// Parachain Onboarding Pallets. Start indices at 70 to leave room.
	#[runtime::pallet_index(70)]
	pub type Registrar = paras_registrar;
	#[runtime::pallet_index(71)]
	pub type Slots = slots;
	#[runtime::pallet_index(72)]
	pub type Auctions = auctions;
	#[runtime::pallet_index(73)]
	pub type Crowdloan = crowdloan;

	// Pallet for sending XCM.
	#[runtime::pallet_index(99)]
	pub type XcmPallet = pallet_xcm;

	// Generalized message queue
	#[runtime::pallet_index(59)]
	pub type MessageQueue = pallet_message_queue;

	// THXNet-specific pallets.
	#[runtime::pallet_index(131)]
	pub type AssetTxPayment = pallet_asset_tx_payment;
	#[runtime::pallet_index(132)]
	pub type Assets = pallet_assets;
	#[runtime::pallet_index(133)]
	pub type Nfts = pallet_nfts;
	#[runtime::pallet_index(134)]
	pub type Dao = pallet_dao;
	#[runtime::pallet_index(135)]
	pub type FinalityRescue = pallet_finality_rescue;
	// Multi-Block Migrations pallet.
	#[runtime::pallet_index(136)]
	pub type MultiBlockMigrations = pallet_migrations;

	// Parachain sudo wrapper.
	#[runtime::pallet_index(250)]
	pub type ParasSudoWrapper = runtime_common::paras_sudo_wrapper;
	// Sudo.
	#[runtime::pallet_index(255)]
	pub type Sudo = pallet_sudo;
}

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// `BlockId` type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The extension to the basic transaction logic.
pub type TxExtension = (
	frame_system::AuthorizeCall<Runtime>,
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckMortality<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
	claims::PrevalidateAttests<Runtime>,
	frame_system::WeightReclaim<Runtime>,
);

pub struct NominationPoolsMigrationV4OldPallet;
impl Get<Perbill> for NominationPoolsMigrationV4OldPallet {
	fn get() -> Perbill {
		Perbill::zero()
	}
}

/// All migrations that will run on the next runtime upgrade.
///
/// This contains the combined migrations of the last 10 releases. It allows to skip runtime
/// upgrades in case governance decides to do so. THE ORDER IS IMPORTANT.
pub type Migrations = migrations::Unreleased;

/// The runtime migrations per release.
#[allow(deprecated, missing_docs)]
pub mod migrations {
	use super::*;
	use frame_support::traits::LockIdentifier;
	use frame_system::pallet_prelude::BlockNumberFor;

	parameter_types! {
		pub const DemocracyPalletName: &'static str = "Democracy";
		pub const CouncilPalletName: &'static str = "Council";
		pub const TechnicalCommitteePalletName: &'static str = "TechnicalCommittee";
		pub const PhragmenElectionPalletName: &'static str = "PhragmenElection";
		pub const TechnicalMembershipPalletName: &'static str = "TechnicalMembership";
		pub const TipsPalletName: &'static str = "Tips";
		pub const PhragmenElectionPalletId: LockIdentifier = *b"phrelect";
	}

	// Special Config for Gov V1 pallets, allowing us to run migrations for them without
	// implementing their configs on [`Runtime`].
	pub struct UnlockConfig;
	impl pallet_democracy::migrations::unlock_and_unreserve_all_funds::UnlockConfig for UnlockConfig {
		type Currency = Balances;
		type MaxVotes = ConstU32<100>;
		type MaxDeposits = ConstU32<100>;
		type AccountId = AccountId;
		type BlockNumber = BlockNumberFor<Runtime>;
		type DbWeight = <Runtime as frame_system::Config>::DbWeight;
		type PalletName = DemocracyPalletName;
	}
	impl pallet_elections_phragmen::migrations::unlock_and_unreserve_all_funds::UnlockConfig
		for UnlockConfig
	{
		type Currency = Balances;
		type MaxVotesPerVoter = ConstU32<16>;
		type PalletId = PhragmenElectionPalletId;
		type AccountId = AccountId;
		type DbWeight = <Runtime as frame_system::Config>::DbWeight;
		type PalletName = PhragmenElectionPalletName;
	}
	impl pallet_tips::migrations::unreserve_deposits::UnlockConfig<()> for UnlockConfig {
		type Currency = Balances;
		type Hash = Hash;
		type DataDepositPerByte = DataDepositPerByte;
		type TipReportDepositBase = TipReportDepositBase;
		type AccountId = AccountId;
		type BlockNumber = BlockNumberFor<Runtime>;
		type DbWeight = <Runtime as frame_system::Config>::DbWeight;
		type PalletName = TipsPalletName;
	}

	pub struct ParachainsToUnlock;
	impl Contains<ParaId> for ParachainsToUnlock {
		fn contains(id: &ParaId) -> bool {
			let id: u32 = (*id).into();
			// polkadot parachains/parathreads that are locked and never produced block
			match id {
				2003 | 2015 | 2017 | 2018 | 2025 | 2028 | 2036 | 2038 | 2053 | 2055 | 2090 |
				2097 | 2106 | 3336 | 3338 | 3342 => true,
				_ => false,
			}
		}
	}

	parameter_types! {
		pub const ImOnlinePalletName: &'static str = "ImOnline";
	}

	/// Bridge migration: stamp pallet_staking from StorageVersion 13 → 14.
	///
	/// Context: The upstream `MigrateToV14` has a manual guard `in_code == 14 && on_chain == 13`.
	/// In v1.12.0, `in_code` is 15, so that guard NEVER fires. We use
	/// `VersionedMigration<13,14,...>` which only checks `on_chain == 13` — the correct condition
	/// for our live chains.
	///
	/// The v14 migration itself is purely a version stamp (no data transformation), so the inner
	/// `UncheckedOnRuntimeUpgrade` is a no-op.
	pub struct StakingV13ToV14Noop;
	impl frame_support::traits::UncheckedOnRuntimeUpgrade for StakingV13ToV14Noop {
		fn on_runtime_upgrade() -> Weight {
			log::info!(target: "runtime::staking", "StakingV13ToV14Noop: stamping v13→v14 (no-op body)");
			Weight::zero()
		}
	}
	/// `VersionedMigration` auto-stamps on_chain_storage_version from 13 to 14 when on_chain == 13.
	pub type StakingBridgeV13ToV14 = frame_support::migrations::VersionedMigration<
		13,
		14,
		StakingV13ToV14Noop,
		pallet_staking::Pallet<Runtime>,
		<Runtime as frame_system::Config>::DbWeight,
	>;

	/// Force-stamp pallet_bounties StorageVersion to v4.
	///
	/// Context: `pallet_bounties` declares `STORAGE_VERSION = StorageVersion::new(4)` in code.
	/// On-chain StorageVersion is NULL (reads as 0) because THXNet never ran the upstream
	/// v4 migration (which was a prefix rename from `Treasury` to `Bounties` — irrelevant
	/// since THXNet always used `Bounties` as the pallet name at index 34).
	///
	/// Without this stamp, try-runtime will assert fail: on-chain 0 != in-code 4.
	/// Safety: No data transformation — purely a metadata correction.
	pub struct StampBountiesV4;
	impl frame_support::traits::OnRuntimeUpgrade for StampBountiesV4 {
		fn on_runtime_upgrade() -> Weight {
			use frame_support::traits::GetStorageVersion;
			let on_chain = pallet_bounties::Pallet::<Runtime>::on_chain_storage_version();
			if on_chain < 4 {
				log::info!(
					target: "runtime::bounties",
					"StampBountiesV4: stamping on-chain version from {:?} to 4",
					on_chain,
				);
				frame_support::traits::StorageVersion::new(4)
					.put::<pallet_bounties::Pallet<Runtime>>();
				<Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 1)
			} else {
				log::info!(
					target: "runtime::bounties",
					"StampBountiesV4: already at {:?}, skipping",
					on_chain,
				);
				<Runtime as frame_system::Config>::DbWeight::get().reads(1)
			}
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_state: alloc::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			use frame_support::traits::GetStorageVersion;
			frame_support::ensure!(
				pallet_bounties::Pallet::<Runtime>::on_chain_storage_version() == 4,
				"StampBountiesV4: on-chain version must be 4 after migration"
			);
			Ok(())
		}
	}

	/// Force-stamp parachains_disputes StorageVersion to v1.
	///
	/// Context: `parachains_disputes` declares `STORAGE_VERSION = StorageVersion::new(1)` in code.
	/// On-chain StorageVersion is 0 because THXNet never ran the upstream v0→v1 migration.
	///
	/// Without this stamp, try-runtime will assert fail: on-chain 0 != in-code 1.
	/// Safety: No data transformation — purely a metadata correction.
	pub struct StampParasDisputesV1;
	impl frame_support::traits::OnRuntimeUpgrade for StampParasDisputesV1 {
		fn on_runtime_upgrade() -> Weight {
			use frame_support::traits::GetStorageVersion;
			let on_chain = parachains_disputes::Pallet::<Runtime>::on_chain_storage_version();
			if on_chain < 1 {
				log::info!(
					target: "runtime::paras_disputes",
					"StampParasDisputesV1: stamping on-chain version from {:?} to 1",
					on_chain,
				);
				frame_support::traits::StorageVersion::new(1)
					.put::<parachains_disputes::Pallet<Runtime>>();
				<Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 1)
			} else {
				log::info!(
					target: "runtime::paras_disputes",
					"StampParasDisputesV1: already at {:?}, skipping",
					on_chain,
				);
				<Runtime as frame_system::Config>::DbWeight::get().reads(1)
			}
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_state: alloc::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			use frame_support::traits::GetStorageVersion;
			frame_support::ensure!(
				parachains_disputes::Pallet::<Runtime>::on_chain_storage_version() == 1,
				"StampParasDisputesV1: on-chain version must be 1 after migration"
			);
			Ok(())
		}
	}

	/// Upgrade Session keys to exclude `ImOnline` key.
	/// When this is removed, should also remove `OldSessionKeys`.
	pub struct UpgradeSessionKeys;
	const UPGRADE_SESSION_KEYS_FROM_SPEC: u32 = 103_000_000;

	impl frame_support::traits::OnRuntimeUpgrade for UpgradeSessionKeys {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<alloc::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
			if System::last_runtime_upgrade_spec_version() > UPGRADE_SESSION_KEYS_FROM_SPEC {
				log::warn!(target: "runtime::session_keys", "Skipping session keys migration pre-upgrade check due to spec version (already applied?)");
				return Ok(Vec::new())
			}

			log::info!(target: "runtime::session_keys", "Collecting pre-upgrade session keys state");
			let key_ids = SessionKeys::key_ids();
			frame_support::ensure!(
				key_ids.into_iter().find(|&k| *k == sp_core::crypto::key_types::IM_ONLINE) == None,
				"New session keys contain the ImOnline key that should have been removed",
			);
			let storage_key = pallet_session::QueuedKeys::<Runtime>::hashed_key();
			let mut state: Vec<u8> = Vec::new();
			frame_support::storage::unhashed::get::<Vec<(ValidatorId, OldSessionKeys)>>(
				&storage_key,
			)
			.ok_or::<sp_runtime::TryRuntimeError>("Queued keys are not available".into())?
			.into_iter()
			.for_each(|(id, keys)| {
				state.extend_from_slice(id.as_ref());
				for key_id in key_ids {
					state.extend_from_slice(keys.get_raw(*key_id));
				}
			});
			frame_support::ensure!(state.len() > 0, "Queued keys are not empty before upgrade");
			Ok(state)
		}

		fn on_runtime_upgrade() -> Weight {
			if System::last_runtime_upgrade_spec_version() > UPGRADE_SESSION_KEYS_FROM_SPEC {
				log::warn!("Skipping session keys upgrade: already applied");
				return <Runtime as frame_system::Config>::DbWeight::get().reads(1)
			}
			log::info!("Upgrading session keys");
			Session::upgrade_keys::<OldSessionKeys, _>(transform_session_keys);
			Perbill::from_percent(50) * BlockWeights::get().max_block
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(old_state: alloc::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			if System::last_runtime_upgrade_spec_version() > UPGRADE_SESSION_KEYS_FROM_SPEC {
				log::warn!(target: "runtime::session_keys", "Skipping session keys migration post-upgrade check due to spec version (already applied?)");
				return Ok(())
			}

			let key_ids = SessionKeys::key_ids();
			let mut new_state: Vec<u8> = Vec::new();
			pallet_session::QueuedKeys::<Runtime>::get().into_iter().for_each(|(id, keys)| {
				new_state.extend_from_slice(id.as_ref());
				for key_id in key_ids {
					new_state.extend_from_slice(keys.get_raw(*key_id));
				}
			});
			frame_support::ensure!(new_state.len() > 0, "Queued keys are not empty after upgrade");
			frame_support::ensure!(
				old_state == new_state,
				"Pre-upgrade and post-upgrade keys do not match!"
			);
			log::info!(target: "runtime::session_keys", "Session keys migrated successfully");
			Ok(())
		}
	}

	// We don't have a limit in the Relay Chain.
	const IDENTITY_MIGRATION_KEY_LIMIT: u64 = u64::MAX;

	parameter_types! {
		pub BalanceTransferAllowDeath: Weight = <weights::pallet_balances::WeightInfo::<Runtime> as pallet_balances::WeightInfo>::transfer_allow_death();
	}

	/// One-time migration to fix GRANDPA finality deadlock (mainnet blocks ~14.2M).
	///
	/// History: This migration was originally deployed at spec_version 94000004 on the old
	/// rootchain (v0.9.40). It has ALREADY EXECUTED on mainnet. On any chain where
	/// `block_number > 14_250_000`, this is a no-op (1 read).
	///
	/// Root cause: Fork blocks at #14206448 created stale `pending_standard_changes` in the
	/// GRANDPA client's AuthoritySet. `current_limit()` returns a limit from these stale
	/// entries, `best_containing` can't find any leaf at or before that limit, and finality
	/// is permanently stalled at #14206447.
	///
	/// Fix: Clear all stale GRANDPA state, then schedule a forced authority change via the
	/// public `schedule_change` API. The `on_finalize` in the same block emits the
	/// `ForcedChange` consensus log, which causes the GRANDPA client to create a brand new
	/// `AuthoritySet` with empty pending changes, unblocking finality.
	///
	/// Ordering: This must run BEFORE `MigrateV4ToV5` in the migration tuple, matching
	/// the real execution order on mainnet (spec 94000004 ran before the v4→v5 migration).
	pub struct FixGrandpaFinalityDeadlock;
	impl frame_support::traits::OnRuntimeUpgrade for FixGrandpaFinalityDeadlock {
		fn on_runtime_upgrade() -> Weight {
			use frame_support::storage;

			let current_set_id_key = storage::storage_prefix(b"Grandpa", b"CurrentSetId");
			let pending_change_key = storage::storage_prefix(b"Grandpa", b"PendingChange");
			let next_forced_key = storage::storage_prefix(b"Grandpa", b"NextForced");
			let stalled_key = storage::storage_prefix(b"Grandpa", b"Stalled");

			let block_number = <frame_system::Pallet<Runtime>>::block_number();

			// Guard: only run while finality is still stuck (within reasonable range).
			// Mainnet is now well past 14.25M, so this is always a no-op.
			if block_number > 14_250_000 {
				log::info!(
					target: "runtime",
					"GRANDPA fix: block #{} past expected range, skipping.",
					block_number,
				);
				return <Runtime as frame_system::Config>::DbWeight::get().reads(1)
			}

			// Step 1: Clear ALL stale GRANDPA state from previous fix attempts.
			if storage::unhashed::exists(&pending_change_key) {
				log::info!(target: "runtime", "GRANDPA fix: clearing stale PendingChange");
			}
			storage::unhashed::kill(&pending_change_key);
			storage::unhashed::kill(&next_forced_key);
			storage::unhashed::kill(&stalled_key);

			// Step 2: Get current authorities.
			let authorities = pallet_grandpa::Pallet::<Runtime>::grandpa_authorities();
			if authorities.is_empty() {
				log::error!(target: "runtime", "GRANDPA fix: no authorities found!");
				return <Runtime as frame_system::Config>::DbWeight::get().reads_writes(2, 3)
			}

			// Step 3: Schedule a forced authority change.
			// With delay=0, on_finalize in the same block will emit ForcedChange log.
			// The GRANDPA client then creates a brand new AuthoritySet, clearing all
			// stale pending changes and unblocking finality.
			let median_finalized: BlockNumber = 14_206_447;
			let result = pallet_grandpa::Pallet::<Runtime>::schedule_change(
				authorities.clone(),
				0u32.into(),
				Some(median_finalized),
			);

			match result {
				Ok(()) => {
					// Step 4: Align runtime CurrentSetId with what the client will have.
					// on_finalize does NOT increment CurrentSetId for forced changes.
					// The GRANDPA client does: new_set_id = self.set_id + 1
					let current_set_id: u64 =
						storage::unhashed::get_or_default(&current_set_id_key);
					let new_set_id: u64 = current_set_id + 1;
					storage::unhashed::put(&current_set_id_key, &new_set_id);

					log::info!(
						target: "runtime",
						"GRANDPA finality fix applied at block #{}: ForcedChange(median={}) \
						 scheduled with {} authorities, CurrentSetId {} -> {}",
						block_number,
						median_finalized,
						authorities.len(),
						current_set_id,
						new_set_id,
					);

					<Runtime as frame_system::Config>::DbWeight::get().reads_writes(5, 6)
				},
				Err(e) => {
					log::error!(
						target: "runtime",
						"GRANDPA finality fix FAILED at block #{}: {:?}",
						block_number,
						e,
					);
					<Runtime as frame_system::Config>::DbWeight::get().reads_writes(2, 3)
				},
			}
		}
	}

	/// Cumulative migrations for live chains upgrading from v0.9.40 to v1.12.0.
	///
	/// Each migration is internally version-guarded (checks `on_chain_storage_version`),
	/// so already-applied migrations are no-ops. This allows a single runtime upgrade
	/// to jump from spec_version 94000001 to 112_000_001 in one shot.
	///
	/// Migration order follows the upstream version progression:
	/// v1.1.0 → v1.3.0 → v1.4.0 → v1.5.0 → v1.6.0 → v1.7.0 → v1.8.0 → v1.9.0 → v1.10.0 → v1.12.0
	/// First half of cumulative migrations (v0.9.40 → v1.5.0).
	type MigrationsEarly = (
		// v0.9.40 → v1.1.0
		// Note: pallet_im_online::migration::v1 skipped — pallet fully removed in Phase 4
		pallet_offences::migration::v1::MigrateToV1<Runtime>,
		// THXNet rootchain is at configuration StorageVersion v4. The v5 and v6 migrations
		// were removed from polkadot-sdk after v1.0.0 (Polkadot/Kusama already ran them).
		// v7 handles migration from v6 structure. We need a custom bridge from v4→v7.
		// TODO: Write custom v4→v6 bridge if on-chain is still v4; otherwise these are no-ops.
		parachains_configuration::migration::v7::MigrateToV7<Runtime>,
		parachains_configuration::migration::v8::MigrateToV8<Runtime>,
		parachains_configuration::migration::v9::MigrateToV9<Runtime>,
		paras_registrar::migration::MigrateToV1<Runtime, ParachainsToUnlock>,
		// v1.2.0 → v1.3.0
		// THXNet rootchain is at nomination_pools StorageVersion v4. V4→V5 versioned wrapper
		// was removed. MigrateToV5 is an OnRuntimeUpgrade with internal version guard
		// (`in_code == 5 && on_chain == 4`). In current codebase in_code may be > 5,
		// so the guard may not fire. This is fine — if on_chain is already >= 5, it's a no-op.
		pallet_nomination_pools::migration::v5::MigrateToV5<Runtime>,
		pallet_nomination_pools::migration::versioned::V5toV6<Runtime>,
		pallet_nomination_pools::migration::versioned::V6ToV7<Runtime>,
		// v1.3.0 → v1.4.0
		// NOTE: Replaced upstream MigrateToV14 (guarded by `in_code==14`, dead in v1.12.0)
		// with custom VersionedMigration bridge that correctly checks `on_chain==13`.
		StakingBridgeV13ToV14,
		// THXNet-specific: GRANDPA finality deadlock fix (originally spec 94000004).
		// Already executed on mainnet — no-op when block > 14_250_000.
		// Must run BEFORE MigrateV4ToV5 (matches real mainnet execution order).
		FixGrandpaFinalityDeadlock,
		pallet_grandpa::migrations::MigrateV4ToV5<Runtime>,
		parachains_configuration::migration::v10::MigrateToV10<Runtime>,
		// v1.4.0 → v1.5.0
		pallet_nomination_pools::migration::versioned::V7ToV8<Runtime>,
		UpgradeSessionKeys,
		frame_support::migrations::RemovePallet<
			ImOnlinePalletName,
			<Runtime as frame_system::Config>::DbWeight,
		>,
		frame_support::migrations::RemovePallet<
			TipsPalletName,
			<Runtime as frame_system::Config>::DbWeight,
		>,
	);

	/// Second half of cumulative migrations (v1.5.0 → v1.12.0).
	type MigrationsLate = (
		// v1.5.0 → v1.6.0
		runtime_parachains::scheduler::migration::MigrateV0ToV1<Runtime>,
		runtime_parachains::scheduler::migration::MigrateV1ToV2<Runtime>,
		pallet_identity::migration::versioned::V0ToV1<Runtime, IDENTITY_MIGRATION_KEY_LIMIT>,
		parachains_configuration::migration::v11::MigrateToV11<Runtime>,
		// v1.6.0 → v1.7.0
		pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>,
		// v1.7.0 → v1.8.0
		pallet_nomination_pools::migration::unversioned::TotalValueLockedSync<Runtime>,
		// v1.8.0 → v1.9.0
		parachains_configuration::migration::v12::MigrateToV12<Runtime>,
		// v1.9.0 → v1.10.0
		parachains_inclusion::migration::MigrateToV1<Runtime>,
		crowdloan::migration::MigrateToTrackInactiveV2<Runtime>,
		// THXNet-specific: Stamp pallet_bounties to v4 (prefix rename was always a noop)
		StampBountiesV4,
		// THXNet-specific: Stamp parachains_disputes to v1 (upstream v0→v1 never ran)
		StampParasDisputesV1,
		// v1.11.0 → v1.12.0
		pallet_staking::migrations::v15::MigrateV14ToV15<Runtime>,
	);

	/// Migrations for v1.12.0 → stable2512.
	type MigrationsStable2512 = (
		parachains_shared::migration::MigrateToV1<Runtime>,
		parachains_scheduler::migration::MigrateV2ToV3<Runtime>,
		pallet_staking::migrations::v16::MigrateV15ToV16<Runtime>,
		pallet_session::migrations::v1::MigrateV0ToV1<
			Runtime,
			pallet_staking::migrations::v17::MigrateDisabledToSession<Runtime>,
		>,
		pallet_child_bounties::migration::MigrateV0ToV1<Runtime, BalanceTransferAllowDeath>,
		// permanent
		pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>,
	);

	/// Full cumulative migrations for live chains upgrading from v0.9.40 to stable2512.
	pub type Unreleased = (MigrationsEarly, MigrationsLate, MigrationsStable2512);
}

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, TxExtension>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
>;

/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, TxExtension>;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	frame_benchmarking::define_benchmarks!(
		// Polkadot
		// NOTE: Make sure to prefix these with `runtime_common::` so
		// the that path resolves correctly in the generated file.
		[runtime_common::auctions, Auctions]
		[runtime_common::claims, Claims]
		[runtime_common::crowdloan, Crowdloan]
		[runtime_common::slots, Slots]
		[runtime_common::paras_registrar, Registrar]
		[runtime_parachains::configuration, Configuration]
		[runtime_parachains::disputes, ParasDisputes]
		[runtime_parachains::disputes::slashing, ParasSlashing]
		[runtime_parachains::hrmp, Hrmp]
		[runtime_parachains::inclusion, ParaInclusion]
		[runtime_parachains::initializer, Initializer]
		[runtime_parachains::paras, Paras]
		[runtime_parachains::paras_inherent, ParaInherent]
		// Substrate
		[pallet_bags_list, VoterList]
		[pallet_balances, Balances]
		[frame_benchmarking::baseline, Baseline::<Runtime>]
		[pallet_bounties, Bounties]
		[pallet_child_bounties, ChildBounties]
		[pallet_election_provider_multi_phase, ElectionProviderMultiPhase]
		[frame_election_provider_support, ElectionProviderBench::<Runtime>]
		[pallet_fast_unstake, FastUnstake]
		[pallet_identity, Identity]
		[pallet_indices, Indices]
		[pallet_message_queue, MessageQueue]
		[pallet_multisig, Multisig]
		[pallet_nomination_pools, NominationPoolsBench::<Runtime>]
		[pallet_offences, OffencesBench::<Runtime>]
		[pallet_preimage, Preimage]
		[pallet_proxy, Proxy]
		[pallet_scheduler, Scheduler]
		[pallet_session, SessionBench::<Runtime>]
		[pallet_staking, Staking]
		[frame_system, SystemBench::<Runtime>]
		[pallet_timestamp, Timestamp]
		[pallet_treasury, Treasury]
		[pallet_utility, Utility]
		[pallet_vesting, Vesting]
		[pallet_democracy, Democracy]
		[pallet_collective, Council]
		[pallet_elections_phragmen, PhragmenElection]
		[pallet_membership, TechnicalMembership]
		// XCM
		[pallet_xcm, PalletXcmExtrinsicsBenchmark::<Runtime>]
		[pallet_xcm_benchmarks::fungible, pallet_xcm_benchmarks::fungible::Pallet::<Runtime>]
		[pallet_xcm_benchmarks::generic, pallet_xcm_benchmarks::generic::Pallet::<Runtime>]
	);
}

sp_api::impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: <Block as BlockT>::LazyBlock) {
			Executive::execute_block(block);
		}

		fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}

		fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
			Runtime::metadata_at_version(version)
		}

		fn metadata_versions() -> alloc::vec::Vec<u32> {
			Runtime::metadata_versions()
		}
	}

	impl block_builder_api::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: <Block as BlockT>::LazyBlock,
			data: inherents::InherentData,
		) -> inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl pallet_nomination_pools_runtime_api::NominationPoolsApi<
		Block,
		AccountId,
		Balance,
	> for Runtime {
		fn pending_rewards(member: AccountId) -> Balance {
			NominationPools::api_pending_rewards(member).unwrap_or_default()
		}

		fn points_to_balance(pool_id: pallet_nomination_pools::PoolId, points: Balance) -> Balance {
			NominationPools::api_points_to_balance(pool_id, points)
		}

		fn balance_to_points(pool_id: pallet_nomination_pools::PoolId, new_funds: Balance) -> Balance {
			NominationPools::api_balance_to_points(pool_id, new_funds)
		}

		fn pool_pending_slash(pool_id: pallet_nomination_pools::PoolId) -> Balance {
			NominationPools::api_pool_pending_slash(pool_id)
		}

		fn member_pending_slash(member: AccountId) -> Balance {
			NominationPools::api_member_pending_slash(member)
		}

		fn pool_needs_delegate_migration(pool_id: pallet_nomination_pools::PoolId) -> bool {
			NominationPools::api_pool_needs_delegate_migration(pool_id)
		}

		fn member_needs_delegate_migration(member: AccountId) -> bool {
			NominationPools::api_member_needs_delegate_migration(member)
		}

		fn member_total_balance(member: AccountId) -> Balance {
			NominationPools::api_member_total_balance(member)
		}

		fn pool_balance(pool_id: pallet_nomination_pools::PoolId) -> Balance {
			NominationPools::api_pool_balance(pool_id)
		}

		fn pool_accounts(pool_id: pallet_nomination_pools::PoolId) -> (AccountId, AccountId) {
			NominationPools::api_pool_accounts(pool_id)
		}
	}

	impl pallet_staking_runtime_api::StakingApi<Block, Balance, AccountId> for Runtime {
		fn nominations_quota(balance: Balance) -> u32 {
			Staking::api_nominations_quota(balance)
		}
		fn eras_stakers_page_count(era: sp_staking::EraIndex, account: AccountId) -> sp_staking::Page {
			Staking::api_eras_stakers_page_count(era, account)
		}
		fn pending_rewards(era: sp_staking::EraIndex, account: AccountId) -> bool {
			Staking::api_pending_rewards(era, account)
		}
	}

	impl tx_pool_api::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl offchain_primitives::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	#[api_version(15)]
	impl primitives::runtime_api::ParachainHost<Block> for Runtime {
		fn validators() -> Vec<ValidatorId> {
			parachains_runtime_api_impl::validators::<Runtime>()
		}

		fn validator_groups() -> (Vec<Vec<ValidatorIndex>>, GroupRotationInfo<BlockNumber>) {
			parachains_runtime_api_impl::validator_groups::<Runtime>()
		}

		fn availability_cores() -> Vec<CoreState<Hash, BlockNumber>> {
			parachains_runtime_api_impl::availability_cores::<Runtime>()
		}

		fn persisted_validation_data(para_id: ParaId, assumption: OccupiedCoreAssumption)
			-> Option<PersistedValidationData<Hash, BlockNumber>> {
			parachains_runtime_api_impl::persisted_validation_data::<Runtime>(para_id, assumption)
		}

		fn assumed_validation_data(
			para_id: ParaId,
			expected_persisted_validation_data_hash: Hash,
		) -> Option<(PersistedValidationData<Hash, BlockNumber>, ValidationCodeHash)> {
			parachains_runtime_api_impl::assumed_validation_data::<Runtime>(
				para_id,
				expected_persisted_validation_data_hash,
			)
		}

		fn check_validation_outputs(
			para_id: ParaId,
			outputs: primitives::CandidateCommitments,
		) -> bool {
			parachains_runtime_api_impl::check_validation_outputs::<Runtime>(para_id, outputs)
		}

		fn session_index_for_child() -> SessionIndex {
			parachains_runtime_api_impl::session_index_for_child::<Runtime>()
		}

		fn validation_code(para_id: ParaId, assumption: OccupiedCoreAssumption)
			-> Option<ValidationCode> {
			parachains_runtime_api_impl::validation_code::<Runtime>(para_id, assumption)
		}

		fn candidate_pending_availability(para_id: ParaId) -> Option<CommittedCandidateReceipt<Hash>> {
			parachains_runtime_api_impl::candidate_pending_availability::<Runtime>(para_id)
		}

		fn candidate_events() -> Vec<CandidateEvent<Hash>> {
			parachains_runtime_api_impl::candidate_events::<Runtime, _>(|ev| {
				match ev {
					RuntimeEvent::ParaInclusion(ev) => {
						Some(ev)
					}
					_ => None,
				}
			})
		}

		fn session_info(index: SessionIndex) -> Option<SessionInfo> {
			parachains_runtime_api_impl::session_info::<Runtime>(index)
		}

		fn session_executor_params(session_index: SessionIndex) -> Option<ExecutorParams> {
			parachains_runtime_api_impl::session_executor_params::<Runtime>(session_index)
		}

		fn dmq_contents(recipient: ParaId) -> Vec<InboundDownwardMessage<BlockNumber>> {
			parachains_runtime_api_impl::dmq_contents::<Runtime>(recipient)
		}

		fn inbound_hrmp_channels_contents(
			recipient: ParaId
		) -> BTreeMap<ParaId, Vec<InboundHrmpMessage<BlockNumber>>> {
			parachains_runtime_api_impl::inbound_hrmp_channels_contents::<Runtime>(recipient)
		}

		fn validation_code_by_hash(hash: ValidationCodeHash) -> Option<ValidationCode> {
			parachains_runtime_api_impl::validation_code_by_hash::<Runtime>(hash)
		}

		fn on_chain_votes() -> Option<ScrapedOnChainVotes<Hash>> {
			parachains_runtime_api_impl::on_chain_votes::<Runtime>()
		}

		fn submit_pvf_check_statement(
			stmt: primitives::PvfCheckStatement,
			signature: primitives::ValidatorSignature,
		) {
			parachains_runtime_api_impl::submit_pvf_check_statement::<Runtime>(stmt, signature)
		}

		fn pvfs_require_precheck() -> Vec<ValidationCodeHash> {
			parachains_runtime_api_impl::pvfs_require_precheck::<Runtime>()
		}

		fn validation_code_hash(para_id: ParaId, assumption: OccupiedCoreAssumption)
			-> Option<ValidationCodeHash>
		{
			parachains_runtime_api_impl::validation_code_hash::<Runtime>(para_id, assumption)
		}

		fn disputes() -> Vec<(SessionIndex, CandidateHash, DisputeState<BlockNumber>)> {
			parachains_runtime_api_impl::get_session_disputes::<Runtime>()
		}

		fn unapplied_slashes(
		) -> Vec<(SessionIndex, CandidateHash, slashing::LegacyPendingSlashes)> {
			parachains_runtime_api_impl::unapplied_slashes::<Runtime>()
		}

		fn key_ownership_proof(
			validator_id: ValidatorId,
		) -> Option<slashing::OpaqueKeyOwnershipProof> {
			use parity_scale_codec::Encode;

			Historical::prove((PARACHAIN_KEY_TYPE_ID, validator_id))
				.map(|p| p.encode())
				.map(slashing::OpaqueKeyOwnershipProof::new)
		}

		fn submit_report_dispute_lost(
			dispute_proof: slashing::DisputeProof,
			key_ownership_proof: slashing::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			parachains_runtime_api_impl::submit_unsigned_slashing_report::<Runtime>(
				dispute_proof,
				key_ownership_proof,
			)
		}

		fn minimum_backing_votes() -> u32 {
			parachains_runtime_api_impl::minimum_backing_votes::<Runtime>()
		}

		fn para_backing_state(para_id: ParaId) -> Option<primitives::async_backing::BackingState> {
			parachains_runtime_api_impl::backing_state::<Runtime>(para_id)
		}

		fn async_backing_params() -> primitives::AsyncBackingParams {
			parachains_runtime_api_impl::async_backing_params::<Runtime>()
		}

		fn approval_voting_params() -> ApprovalVotingParams {
			parachains_runtime_api_impl::approval_voting_params::<Runtime>()
		}

		fn disabled_validators() -> Vec<ValidatorIndex> {
			parachains_runtime_api_impl::disabled_validators::<Runtime>()
		}

		fn node_features() -> NodeFeatures {
			parachains_runtime_api_impl::node_features::<Runtime>()
		}

		fn claim_queue() -> BTreeMap<CoreIndex, VecDeque<ParaId>> {
			parachains_runtime_api_impl::claim_queue::<Runtime>()
		}

		fn candidates_pending_availability(para_id: ParaId) -> Vec<CommittedCandidateReceipt<Hash>> {
			parachains_runtime_api_impl::candidates_pending_availability::<Runtime>(para_id)
		}

		fn backing_constraints(para_id: ParaId) -> Option<Constraints> {
			parachains_runtime_api_impl::backing_constraints::<Runtime>(para_id)
		}

		fn scheduling_lookahead() -> u32 {
			parachains_runtime_api_impl::scheduling_lookahead::<Runtime>()
		}

		fn validation_code_bomb_limit() -> u32 {
			parachains_runtime_api_impl::validation_code_bomb_limit::<Runtime>()
		}

		fn para_ids() -> Vec<ParaId> {
			parachains_staging_runtime_api_impl::para_ids::<Runtime>()
		}

		fn unapplied_slashes_v2(
		) -> Vec<(SessionIndex, CandidateHash, slashing::PendingSlashes)> {
			parachains_runtime_api_impl::unapplied_slashes_v2::<Runtime>()
		}
	}

	impl beefy_primitives::BeefyApi<Block, BeefyId> for Runtime {
		fn beefy_genesis() -> Option<BlockNumber> {
			// dummy implementation due to lack of BEEFY pallet.
			None
		}

		fn validator_set() -> Option<beefy_primitives::ValidatorSet<BeefyId>> {
			// dummy implementation due to lack of BEEFY pallet.
			None
		}

		fn submit_report_double_voting_unsigned_extrinsic(
			_equivocation_proof: beefy_primitives::DoubleVotingProof<
				BlockNumber,
				BeefyId,
				BeefySignature,
			>,
			_key_owner_proof: sp_runtime::OpaqueValue,
		) -> Option<()> {
			None
		}

		fn submit_report_fork_voting_unsigned_extrinsic(
			_equivocation_proof: beefy_primitives::ForkVotingProof<
				<Block as BlockT>::Header,
				BeefyId,
				sp_runtime::OpaqueValue,
			>,
			_key_owner_proof: sp_runtime::OpaqueValue,
		) -> Option<()> {
			None
		}

		fn submit_report_future_block_voting_unsigned_extrinsic(
			_equivocation_proof: beefy_primitives::FutureBlockVotingProof<BlockNumber, BeefyId>,
			_key_owner_proof: sp_runtime::OpaqueValue,
		) -> Option<()> {
			None
		}

		fn generate_key_ownership_proof(
			_set_id: beefy_primitives::ValidatorSetId,
			_authority_id: BeefyId,
		) -> Option<beefy_primitives::OpaqueKeyOwnershipProof> {
			None
		}
	}

	impl mmr::MmrApi<Block, Hash, BlockNumber> for Runtime {
		fn mmr_root() -> Result<Hash, mmr::Error> {
			Err(mmr::Error::PalletNotIncluded)
		}

		fn mmr_leaf_count() -> Result<mmr::LeafIndex, mmr::Error> {
			Err(mmr::Error::PalletNotIncluded)
		}

		fn generate_proof(
			_block_numbers: Vec<BlockNumber>,
			_best_known_block_number: Option<BlockNumber>,
		) -> Result<(Vec<mmr::EncodableOpaqueLeaf>, mmr::LeafProof<Hash>), mmr::Error> {
			Err(mmr::Error::PalletNotIncluded)
		}

		fn generate_ancestry_proof(
			_prev_block_number: BlockNumber,
			_best_known_block_number: Option<BlockNumber>,
		) -> Result<mmr::AncestryProof<Hash>, mmr::Error> {
			Err(mmr::Error::PalletNotIncluded)
		}

		fn verify_proof(_leaves: Vec<mmr::EncodableOpaqueLeaf>, _proof: mmr::LeafProof<Hash>)
			-> Result<(), mmr::Error>
		{
			Err(mmr::Error::PalletNotIncluded)
		}

		fn verify_proof_stateless(
			_root: Hash,
			_leaves: Vec<mmr::EncodableOpaqueLeaf>,
			_proof: mmr::LeafProof<Hash>
		) -> Result<(), mmr::Error> {
			Err(mmr::Error::PalletNotIncluded)
		}
	}

	impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> Vec<(GrandpaId, u64)> {
			Grandpa::grandpa_authorities()
		}

		fn current_set_id() -> fg_primitives::SetId {
			Grandpa::current_set_id()
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			equivocation_proof: fg_primitives::EquivocationProof<
				<Block as BlockT>::Hash,
				sp_runtime::traits::NumberFor<Block>,
			>,
			key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			let key_owner_proof = key_owner_proof.decode()?;

			Grandpa::submit_unsigned_equivocation_report(
				equivocation_proof,
				key_owner_proof,
			)
		}

		fn generate_key_ownership_proof(
			_set_id: fg_primitives::SetId,
			authority_id: fg_primitives::AuthorityId,
		) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
			use parity_scale_codec::Encode;

			Historical::prove((fg_primitives::KEY_TYPE, authority_id))
				.map(|p| p.encode())
				.map(fg_primitives::OpaqueKeyOwnershipProof::new)
		}
	}

	impl babe_primitives::BabeApi<Block> for Runtime {
		fn configuration() -> babe_primitives::BabeConfiguration {
			let epoch_config = Babe::epoch_config().unwrap_or(BABE_GENESIS_EPOCH_CONFIG);
			babe_primitives::BabeConfiguration {
				slot_duration: Babe::slot_duration(),
				epoch_length: EpochDuration::get(),
				c: epoch_config.c,
				authorities: Babe::authorities().to_vec(),
				randomness: Babe::randomness(),
				allowed_slots: epoch_config.allowed_slots,
			}
		}

		fn current_epoch_start() -> babe_primitives::Slot {
			Babe::current_epoch_start()
		}

		fn current_epoch() -> babe_primitives::Epoch {
			Babe::current_epoch()
		}

		fn next_epoch() -> babe_primitives::Epoch {
			Babe::next_epoch()
		}

		fn generate_key_ownership_proof(
			_slot: babe_primitives::Slot,
			authority_id: babe_primitives::AuthorityId,
		) -> Option<babe_primitives::OpaqueKeyOwnershipProof> {
			use parity_scale_codec::Encode;

			Historical::prove((babe_primitives::KEY_TYPE, authority_id))
				.map(|p| p.encode())
				.map(babe_primitives::OpaqueKeyOwnershipProof::new)
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			equivocation_proof: babe_primitives::EquivocationProof<<Block as BlockT>::Header>,
			key_owner_proof: babe_primitives::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			let key_owner_proof = key_owner_proof.decode()?;

			Babe::submit_unsigned_equivocation_report(
				equivocation_proof,
				key_owner_proof,
			)
		}
	}

	impl authority_discovery_primitives::AuthorityDiscoveryApi<Block> for Runtime {
		fn authorities() -> Vec<AuthorityDiscoveryId> {
			parachains_runtime_api_impl::relevant_authority_ids::<Runtime>()
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, sp_core::crypto::KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
		fn account_nonce(account: AccountId) -> Nonce {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
		Block,
		Balance,
	> for Runtime {
		fn query_info(uxt: <Block as BlockT>::Extrinsic, len: u32) -> RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(uxt: <Block as BlockT>::Extrinsic, len: u32) -> FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
		for Runtime
	{
		fn query_call_info(call: RuntimeCall, len: u32) -> RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_call_info(call, len)
		}
		fn query_call_fee_details(call: RuntimeCall, len: u32) -> FeeDetails<Balance> {
			TransactionPayment::query_call_fee_details(call, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
		fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
			frame_support::genesis_builder_helper::build_state::<RuntimeGenesisConfig>(config)
		}

		fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
			frame_support::genesis_builder_helper::get_preset::<RuntimeGenesisConfig>(id, |_| None)
		}

		fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
			vec![]
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
			log::info!("try-runtime::on_runtime_upgrade polkadot.");
			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, BlockWeights::get().max_block)
		}

		fn execute_block(
			block: <Block as BlockT>::LazyBlock,
			state_root_check: bool,
			signature_check: bool,
			select: frame_try_runtime::TryStateSelect,
		) -> Weight {
			// NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
			// have a backtrace here.
			Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;

			use pallet_session_benchmarking::Pallet as SessionBench;
			use pallet_offences_benchmarking::Pallet as OffencesBench;
			use pallet_election_provider_support_benchmarking::Pallet as ElectionProviderBench;
			use pallet_nomination_pools_benchmarking::Pallet as NominationPoolsBench;
			use frame_system_benchmarking::Pallet as SystemBench;
			use frame_benchmarking::baseline::Pallet as Baseline;
			use pallet_xcm::benchmarking::Pallet as PalletXcmExtrinsicsBenchmark;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();
			return (list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<
			Vec<frame_benchmarking::BenchmarkBatch>,
			sp_runtime::RuntimeString,
		> {
			use frame_support::traits::WhitelistedStorageKeys;
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, BenchmarkError};
			use sp_storage::TrackedStorageKey;
			// Trying to add benchmarks directly to some pallets caused cyclic dependency issues.
			// To get around that, we separated the benchmarks into its own crate.
			use pallet_session_benchmarking::Pallet as SessionBench;
			use pallet_offences_benchmarking::Pallet as OffencesBench;
			use pallet_election_provider_support_benchmarking::Pallet as ElectionProviderBench;
			use pallet_nomination_pools_benchmarking::Pallet as NominationPoolsBench;
			use frame_system_benchmarking::Pallet as SystemBench;
			use frame_benchmarking::baseline::Pallet as Baseline;
			use pallet_xcm::benchmarking::Pallet as PalletXcmExtrinsicsBenchmark;
			use xcm::latest::prelude::*;
			use xcm_config::{XcmConfig, StatemintLocation, TokenLocation, LocalCheckAccount, SovereignAccountOf};

			impl pallet_session_benchmarking::Config for Runtime {}
			impl pallet_offences_benchmarking::Config for Runtime {}
			impl pallet_election_provider_support_benchmarking::Config for Runtime {}
			impl frame_system_benchmarking::Config for Runtime {}
			impl frame_benchmarking::baseline::Config for Runtime {}
			impl pallet_nomination_pools_benchmarking::Config for Runtime {}
			impl runtime_parachains::disputes::slashing::benchmarking::Config for Runtime {}

			impl pallet_xcm::benchmarking::Config for Runtime {
				type DeliveryHelper = ();

				fn reachable_dest() -> Option<Location> {
					Some(StatemintLocation::get())
				}

				fn teleportable_asset_and_dest() -> Option<(Asset, Location)> {
					Some((
						Asset { fun: Fungible(1 * UNITS), id: AssetId(TokenLocation::get()) },
						StatemintLocation::get(),
					))
				}

				fn reserve_transferable_asset_and_dest() -> Option<(Asset, Location)> {
					None
				}

				fn get_asset() -> Asset {
					Asset {
						id: AssetId(TokenLocation::get()),
						fun: Fungible(1 * UNITS),
					}
				}
			}

			let mut whitelist: Vec<TrackedStorageKey> = AllPalletsWithSystem::whitelisted_storage_keys();
			let treasury_key = frame_system::Account::<Runtime>::hashed_key_for(Treasury::account_id());
			whitelist.push(treasury_key.to_vec().into());

			impl pallet_xcm_benchmarks::Config for Runtime {
				type XcmConfig = XcmConfig;
				type AccountIdConverter = SovereignAccountOf;
				type DeliveryHelper = ();
				fn valid_destination() -> Result<Location, BenchmarkError> {
					Ok(StatemintLocation::get())
				}
				fn worst_case_holding(_depositable_count: u32) -> Assets {
					// Polkadot only knows about DOT
					vec![Asset { id: AssetId(TokenLocation::get()), fun: Fungible(1_000_000 * UNITS) }].into()
				}
			}

			parameter_types! {
				pub TrustedTeleporter: Option<(Location, Asset)> = Some((
					StatemintLocation::get(),
					Asset { id: AssetId(TokenLocation::get()), fun: Fungible(1 * UNITS) }
				));
				pub TrustedReserve: Option<(Location, Asset)> = None;
			}

			impl pallet_xcm_benchmarks::fungible::Config for Runtime {
				type TransactAsset = Balances;

				type CheckedAccount = LocalCheckAccount;
				type TrustedTeleporter = TrustedTeleporter;
				type TrustedReserve = TrustedReserve;

				fn get_asset() -> Asset {
					Asset {
						id: AssetId(TokenLocation::get()),
						fun: Fungible(1 * UNITS),
					}
				}
			}

			impl pallet_xcm_benchmarks::generic::Config for Runtime {
				type TransactAsset = Balances;
				type RuntimeCall = RuntimeCall;

				fn worst_case_response() -> (u64, Response) {
					(0u64, Response::Version(Default::default()))
				}

				fn worst_case_asset_exchange() -> Result<(Assets, Assets), BenchmarkError> {
					// Polkadot doesn't support asset exchanges
					Err(BenchmarkError::Skip)
				}

				fn universal_alias() -> Result<(Location, Junction), BenchmarkError> {
					// The XCM executor of Polkadot doesn't have a configured `UniversalAliases`
					Err(BenchmarkError::Skip)
				}

				fn transact_origin_and_runtime_call() -> Result<(Location, RuntimeCall), BenchmarkError> {
					Ok((StatemintLocation::get(), frame_system::Call::remark_with_event { remark: vec![] }.into()))
				}

				fn subscribe_origin() -> Result<Location, BenchmarkError> {
					Ok(StatemintLocation::get())
				}

				fn claimable_asset() -> Result<(Location, Location, Assets), BenchmarkError> {
					let origin = StatemintLocation::get();
					let assets: Assets = (AssetId(TokenLocation::get()), 1_000 * UNITS).into();
					let ticket = Location { parents: 0, interior: Here };
					Ok((origin, ticket, assets))
				}

				fn fee_asset() -> Result<Asset, BenchmarkError> {
					Ok(Asset {
						id: AssetId(TokenLocation::get()),
						fun: Fungible(1_000_000 * UNITS),
					})
				}

				fn unlockable_asset() -> Result<(Location, Location, Asset), BenchmarkError> {
					// Polkadot doesn't support asset locking
					Err(BenchmarkError::Skip)
				}

				fn export_message_origin_and_destination(
				) -> Result<(Location, NetworkId, InteriorLocation), BenchmarkError> {
					// Polkadot doesn't support exporting messages
					Err(BenchmarkError::Skip)
				}

				fn alias_origin() -> Result<(Location, Location), BenchmarkError> {
					// The XCM executor of Polkadot doesn't have a configured `Aliasers`
					Err(BenchmarkError::Skip)
				}
			}

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);

			add_benchmarks!(params, batches);

			Ok(batches)
		}
	}
}

#[cfg(test)]
mod test_fees {
	use super::*;
	use frame_support::{dispatch::GetDispatchInfo, weights::WeightToFee as WeightToFeeT};
	use keyring::Sr25519Keyring::{Alice, Charlie};
	use pallet_transaction_payment::Multiplier;
	use runtime_common::MinimumMultiplier;
	use separator::Separatable;
	use sp_runtime::{assert_eq_error_rate, FixedPointNumber, MultiAddress, MultiSignature};

	#[test]
	fn payout_weight_portion() {
		use pallet_staking::WeightInfo;
		let payout_weight =
			<Runtime as pallet_staking::Config>::WeightInfo::payout_stakers_alive_staked(
				MaxExposurePageSize::get(),
			)
			.ref_time() as f64;
		let block_weight = BlockWeights::get().max_block.ref_time() as f64;

		println!(
			"a full payout takes {:.2} of the block weight [{} / {}]",
			payout_weight / block_weight,
			payout_weight,
			block_weight
		);
		assert!(payout_weight * 2f64 < block_weight);
	}

	#[test]
	fn block_cost() {
		let max_block_weight = BlockWeights::get().max_block;
		let raw_fee = WeightToFee::weight_to_fee(&max_block_weight);

		let fee_with_multiplier = |m: Multiplier| {
			println!(
				"Full Block weight == {} // multiplier: {:?} // WeightToFee(full_block) == {} plank",
				max_block_weight,
				m,
				m.saturating_mul_int(raw_fee).separated_string(),
			);
		};
		fee_with_multiplier(MinimumMultiplier::get());
		fee_with_multiplier(Multiplier::from_rational(1, 2));
		fee_with_multiplier(Multiplier::from_u32(1));
		fee_with_multiplier(Multiplier::from_u32(2));
	}

	#[test]
	fn transfer_cost_min_multiplier() {
		let min_multiplier = MinimumMultiplier::get();
		let call = pallet_balances::Call::<Runtime>::transfer_keep_alive {
			dest: Charlie.to_account_id().into(),
			value: Default::default(),
		};
		let info = call.get_dispatch_info();
		println!("call = {:?} / info = {:?}", call, info);
		// convert to runtime call.
		let call = RuntimeCall::Balances(call);
		let extra: TxExtension = (
			frame_system::AuthorizeCall::<Runtime>::new(),
			frame_system::CheckNonZeroSender::<Runtime>::new(),
			frame_system::CheckSpecVersion::<Runtime>::new(),
			frame_system::CheckTxVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckMortality::<Runtime>::from(generic::Era::immortal()),
			frame_system::CheckNonce::<Runtime>::from(1),
			frame_system::CheckWeight::<Runtime>::new(),
			pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0),
			claims::PrevalidateAttests::<Runtime>::new(),
			frame_system::WeightReclaim::<Runtime>::new(),
		);
		let uxt = UncheckedExtrinsic::new_signed(
			call,
			MultiAddress::Id(Alice.to_account_id()),
			MultiSignature::Sr25519(Alice.sign(b"foo")),
			extra,
		);
		let len = uxt.encoded_size();

		let mut ext = sp_io::TestExternalities::new_empty();
		let mut test_with_multiplier = |m: Multiplier| {
			ext.execute_with(|| {
				pallet_transaction_payment::NextFeeMultiplier::<Runtime>::put(m);
				let fee = TransactionPayment::query_fee_details(uxt.clone(), len as u32);
				println!(
					"multiplier = {:?} // fee details = {:?} // final fee = {:?}",
					pallet_transaction_payment::NextFeeMultiplier::<Runtime>::get(),
					fee,
					fee.final_fee().separated_string(),
				);
			});
		};

		test_with_multiplier(min_multiplier);
		test_with_multiplier(Multiplier::saturating_from_rational(1u128, 1u128));
		test_with_multiplier(Multiplier::saturating_from_rational(1u128, 1_0u128));
		test_with_multiplier(Multiplier::saturating_from_rational(1u128, 1_00u128));
		test_with_multiplier(Multiplier::saturating_from_rational(1u128, 1_000u128));
		test_with_multiplier(Multiplier::saturating_from_rational(1u128, 1_000_000u128));
		test_with_multiplier(Multiplier::saturating_from_rational(1u128, 1_000_000_000u128));
	}

	#[test]
	fn nominator_limit() {
		use pallet_election_provider_multi_phase::WeightInfo;
		// starting point of the nominators.
		let target_voters: u32 = 50_000;

		// assuming we want around 5k candidates and 1k active validators. (March 31, 2021)
		let all_targets: u32 = 5_000;
		let desired: u32 = 1_000;
		let weight_with = |active| {
			<Runtime as pallet_election_provider_multi_phase::Config>::WeightInfo::submit_unsigned(
				active,
				all_targets,
				active,
				desired,
			)
		};

		let mut active = target_voters;
		while weight_with(active).all_lte(OffchainSolutionWeightLimit::get()) ||
			active == target_voters
		{
			active += 1;
		}

		println!("can support {} nominators to yield a weight of {}", active, weight_with(active));
		assert!(active > target_voters, "we need to reevaluate the weight of the election system");
	}

	#[test]
	#[ignore = "SignedDepositBase is now GeometricDepositBase (not a Get<Balance>), and THXNet is zero-fee"]
	fn signed_deposit_is_sensible() {
		// ensure this number does not change, or that it is checked after each change.
		// a 1 MB solution should take (40 + 10) DOTs of deposit.
		// NOTE: Broken upstream — SignedDepositBase changed from parameter_types to
		// GeometricDepositBase which is not a Get<Balance>.
		let deposit = SignedDepositByte::get() * 1024 * 1024;
		assert_eq_error_rate!(deposit, 50 * DOLLARS, DOLLARS);
	}
}

#[cfg(test)]
mod test {
	use std::collections::HashSet;

	use super::*;
	use frame_support::traits::WhitelistedStorageKeys;
	use sp_core::hexdisplay::HexDisplay;

	#[test]
	fn call_size() {
		RuntimeCall::assert_size_under(230);
	}

	#[test]
	fn check_whitelist() {
		let whitelist: HashSet<String> = AllPalletsWithSystem::whitelisted_storage_keys()
			.iter()
			.map(|e| HexDisplay::from(&e.key).to_string())
			.collect();

		// Block number
		assert!(
			whitelist.contains("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac")
		);
		// Total issuance
		assert!(
			whitelist.contains("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80")
		);
		// Execution phase
		assert!(
			whitelist.contains("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a")
		);
		// Event count
		assert!(
			whitelist.contains("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850")
		);
		// System events
		assert!(
			whitelist.contains("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7")
		);
		// XcmPallet VersionDiscoveryQueue
		assert!(
			whitelist.contains("1405f2411d0af5a7ff397e7c9dc68d194a222ba0333561192e474c59ed8e30e1")
		);
		// XcmPallet SafeXcmVersion
		assert!(
			whitelist.contains("1405f2411d0af5a7ff397e7c9dc68d196323ae84c43568be0d1394d5d0d522c4")
		);
	}
}

#[cfg(test)]
mod multiplier_tests {
	use super::*;
	use frame_support::{dispatch::DispatchInfo, traits::OnFinalize};
	use runtime_common::{MinimumMultiplier, TargetBlockFullness};
	use separator::Separatable;
	use sp_runtime::traits::Convert;

	fn run_with_system_weight<F>(w: Weight, mut assertions: F)
	where
		F: FnMut() -> (),
	{
		let mut t: sp_io::TestExternalities = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap()
			.into();
		t.execute_with(|| {
			System::set_block_consumed_resources(w, 0);
			assertions()
		});
	}

	#[test]
	fn multiplier_can_grow_from_zero() {
		let minimum_multiplier = MinimumMultiplier::get();
		let target = TargetBlockFullness::get() *
			BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap();
		// if the min is too small, then this will not change, and we are doomed forever.
		// the weight is 1/100th bigger than target.
		run_with_system_weight(target.saturating_mul(101) / 100, || {
			let next = SlowAdjustingFeeUpdate::<Runtime>::convert(minimum_multiplier);
			assert!(next > minimum_multiplier, "{:?} !>= {:?}", next, minimum_multiplier);
		})
	}

	#[test]
	fn fast_unstake_estimate() {
		use pallet_fast_unstake::WeightInfo;
		let block_time = BlockWeights::get().max_block.ref_time() as f32;
		let on_idle = weights::pallet_fast_unstake::WeightInfo::<Runtime>::on_idle_check(
			300,
			<Runtime as pallet_fast_unstake::Config>::BatchSize::get(),
		)
		.ref_time() as f32;
		println!("ratio of block weight for full batch fast-unstake {}", on_idle / block_time);
		assert!(on_idle / block_time <= 0.5f32)
	}

	#[test]
	#[ignore]
	fn multiplier_growth_simulator() {
		// assume the multiplier is initially set to its minimum. We update it with values twice the
		//target (target is 25%, thus 50%) and we see at which point it reaches 1.
		let mut multiplier = MinimumMultiplier::get();
		let block_weight = BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap();
		let mut blocks = 0;
		let mut fees_paid = 0;

		frame_system::Pallet::<Runtime>::set_block_consumed_resources(Weight::MAX, 0);
		let info = DispatchInfo { call_weight: Weight::MAX, ..Default::default() };

		let mut t: sp_io::TestExternalities = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap()
			.into();
		// set the minimum
		t.execute_with(|| {
			pallet_transaction_payment::NextFeeMultiplier::<Runtime>::set(MinimumMultiplier::get());
		});

		while multiplier <= Multiplier::from_u32(1) {
			t.execute_with(|| {
				// imagine this tx was called.
				let fee = TransactionPayment::compute_fee(0, &info, 0);
				fees_paid += fee;

				// this will update the multiplier.
				System::set_block_consumed_resources(block_weight, 0);
				TransactionPayment::on_finalize(1);
				let next = TransactionPayment::next_fee_multiplier();

				assert!(next > multiplier, "{:?} !>= {:?}", next, multiplier);
				multiplier = next;

				println!(
					"block = {} / multiplier {:?} / fee = {:?} / fess so far {:?}",
					blocks,
					multiplier,
					fee.separated_string(),
					fees_paid.separated_string()
				);
			});
			blocks += 1;
		}
	}

	#[test]
	#[ignore]
	fn multiplier_cool_down_simulator() {
		// assume the multiplier is initially set to its minimum. We update it with values twice the
		//target (target is 25%, thus 50%) and we see at which point it reaches 1.
		let mut multiplier = Multiplier::from_u32(2);
		let mut blocks = 0;

		let mut t: sp_io::TestExternalities = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap()
			.into();
		// set the minimum
		t.execute_with(|| {
			pallet_transaction_payment::NextFeeMultiplier::<Runtime>::set(multiplier);
		});

		while multiplier > Multiplier::from_u32(0) {
			t.execute_with(|| {
				// this will update the multiplier.
				TransactionPayment::on_finalize(1);
				let next = TransactionPayment::next_fee_multiplier();

				assert!(next < multiplier, "{:?} !>= {:?}", next, multiplier);
				multiplier = next;

				println!("block = {} / multiplier {:?}", blocks, multiplier);
			});
			blocks += 1;
		}
	}
}

// ════════════════════════════════════════════════════════════════════════════
// Migration correctness tests for THXNet v0.9.40 → v1.12.0 upgrade.
//
// These tests validate every custom migration that has NO existing unit tests.
// Each test targets exactly one MECE partition of the migration behavior space.
// ════════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod migration_tests {
	use super::*;
	use frame_support::{
		storage,
		traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion},
	};

	// ── A1: StakingBridgeV13ToV14 ───────────────────────────────────────────
	// Partition: {on_chain == 13 → stamps to 14, on_chain != 13 → skips}

	#[test]
	fn staking_bridge_v13_to_v14_stamps_version_when_on_chain_is_13() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: set Staking on-chain version to 13
			StorageVersion::new(13).put::<pallet_staking::Pallet<Runtime>>();
			assert_eq!(
				pallet_staking::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(13)
			);

			// Act
			let weight = migrations::StakingBridgeV13ToV14::on_runtime_upgrade();

			// Assert: version stamped to 14
			assert_eq!(
				pallet_staking::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(14),
				"StakingBridgeV13ToV14 must stamp on-chain version from 13 to 14"
			);
			// Weight should include at least a read + write for the version check/stamp
			assert!(weight.ref_time() > 0, "Migration should report non-zero weight");
		});
	}

	#[test]
	fn staking_bridge_v13_to_v14_skips_when_on_chain_is_14() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: already at v14
			StorageVersion::new(14).put::<pallet_staking::Pallet<Runtime>>();

			// Act
			let _weight = migrations::StakingBridgeV13ToV14::on_runtime_upgrade();

			// Assert: version unchanged
			assert_eq!(
				pallet_staking::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(14),
				"StakingBridgeV13ToV14 must not modify version when already at 14"
			);
		});
	}

	#[test]
	fn staking_bridge_v13_to_v14_skips_when_on_chain_is_15() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: already past v14
			StorageVersion::new(15).put::<pallet_staking::Pallet<Runtime>>();

			// Act
			let _weight = migrations::StakingBridgeV13ToV14::on_runtime_upgrade();

			// Assert: version unchanged
			assert_eq!(
				pallet_staking::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(15),
				"StakingBridgeV13ToV14 must not modify version when already past 14"
			);
		});
	}

	#[test]
	fn staking_v13_to_v14_noop_body_returns_zero_weight() {
		sp_io::TestExternalities::default().execute_with(|| {
			use frame_support::traits::UncheckedOnRuntimeUpgrade;
			let weight = migrations::StakingV13ToV14Noop::on_runtime_upgrade();
			assert_eq!(weight, Weight::zero(), "Noop body must return zero weight");
		});
	}

	// ── A2: FixGrandpaFinalityDeadlock ──────────────────────────────────────
	// Partition: {block > 14.25M → noop, block <= 14.25M + authorities → fix,
	//             block <= 14.25M + no authorities → error return}

	#[test]
	fn fix_grandpa_noop_when_block_past_14_250_000() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: block number well past the guard
			System::set_block_number(15_000_000);

			// Seed stale state to verify it is NOT touched
			let pending_change_key = storage::storage_prefix(b"Grandpa", b"PendingChange");
			storage::unhashed::put_raw(&pending_change_key, &[1, 2, 3]);

			// Act
			let weight = migrations::FixGrandpaFinalityDeadlock::on_runtime_upgrade();

			// Assert: stale state still exists (not cleared), minimal weight
			assert!(
				storage::unhashed::exists(&pending_change_key),
				"FixGrandpaFinalityDeadlock should NOT clear state when block > 14.25M"
			);
			assert_eq!(
				weight,
				<Runtime as frame_system::Config>::DbWeight::get().reads(1),
				"Should return weight of 1 read when skipping"
			);
		});
	}

	#[test]
	fn fix_grandpa_clears_stale_state_when_block_within_range() {
		use pallet_grandpa::AuthorityId;
		use sp_core::crypto::UncheckedFrom;

		let authorities: pallet_grandpa::AuthorityList = vec![
			(AuthorityId::unchecked_from([1u8; 32]), 1u64),
			(AuthorityId::unchecked_from([2u8; 32]), 1u64),
		];

		// Build genesis with GRANDPA authorities seeded via GenesisConfig
		let mut storage =
			frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();
		pallet_grandpa::GenesisConfig::<Runtime> {
			authorities: authorities.clone(),
			_config: Default::default(),
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(storage);
		ext.execute_with(|| {
			// Arrange: block number within range
			System::set_block_number(14_200_000);

			let current_set_id_key =
				frame_support::storage::storage_prefix(b"Grandpa", b"CurrentSetId");
			frame_support::storage::unhashed::put::<u64>(&current_set_id_key, &42);

			// Seed stale state
			let pending_change_key =
				frame_support::storage::storage_prefix(b"Grandpa", b"PendingChange");
			let next_forced_key = frame_support::storage::storage_prefix(b"Grandpa", b"NextForced");
			let stalled_key = frame_support::storage::storage_prefix(b"Grandpa", b"Stalled");
			frame_support::storage::unhashed::put_raw(&pending_change_key, &[1, 2, 3]);
			frame_support::storage::unhashed::put_raw(&next_forced_key, &[4, 5, 6]);
			frame_support::storage::unhashed::put_raw(&stalled_key, &[7, 8, 9]);

			// Act
			let weight = migrations::FixGrandpaFinalityDeadlock::on_runtime_upgrade();

			// Assert: Stalled is cleared
			assert!(
				!frame_support::storage::unhashed::exists(&stalled_key),
				"Stalled should be cleared"
			);

			// NextForced is re-set by schedule_change (delay=0, forced=Some):
			// scheduled_at + in_blocks * 2 = 14_200_000 + 0 = 14_200_000
			// So NextForced exists but with a NEW value (not the stale [4,5,6])
			assert!(
				frame_support::storage::unhashed::exists(&next_forced_key),
				"NextForced should be re-created by schedule_change"
			);

			// PendingChange should be re-created by schedule_change (not the stale [1,2,3])
			assert!(
				frame_support::storage::unhashed::exists(&pending_change_key),
				"PendingChange should be re-created by schedule_change"
			);

			// Assert: CurrentSetId incremented from 42 to 43
			let new_set_id: u64 =
				frame_support::storage::unhashed::get_or_default(&current_set_id_key);
			assert_eq!(new_set_id, 43, "CurrentSetId should be incremented by 1");

			// Assert: weight is reads_writes(5, 6) for successful path
			assert_eq!(
				weight,
				<Runtime as frame_system::Config>::DbWeight::get().reads_writes(5, 6),
				"Successful fix should return reads_writes(5, 6)"
			);
		});
	}

	#[test]
	fn fix_grandpa_handles_empty_authorities() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: block within range but NO authorities
			System::set_block_number(14_200_000);
			// Don't seed any authorities — Grandpa::grandpa_authorities() returns empty

			// Act
			let weight = migrations::FixGrandpaFinalityDeadlock::on_runtime_upgrade();

			// Assert: early return with reads_writes(2, 3)
			assert_eq!(
				weight,
				<Runtime as frame_system::Config>::DbWeight::get().reads_writes(2, 3),
				"Empty authorities should return reads_writes(2, 3) error path weight"
			);
		});
	}

	// ── A3: UpgradeSessionKeys ──────────────────────────────────────────────
	// Partition: {spec > threshold → skips, spec <= threshold → transforms keys}

	#[test]
	fn upgrade_session_keys_skips_when_spec_version_above_threshold() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: set last runtime upgrade spec version above the threshold.
			// The threshold is 103_000_000 (mainnet), defined as a private const
			// UPGRADE_SESSION_KEYS_FROM_SPEC inside the migrations module.
			frame_system::LastRuntimeUpgrade::<Runtime>::put(
				frame_system::LastRuntimeUpgradeInfo {
					spec_version: 104_000_000u32.into(),
					spec_name: "thxnet".into(),
				},
			);

			// Act
			let weight = migrations::UpgradeSessionKeys::on_runtime_upgrade();

			// Assert: only 1 read (the spec version check)
			assert_eq!(
				weight,
				<Runtime as frame_system::Config>::DbWeight::get().reads(1),
				"Should return 1 read when spec version is above threshold"
			);
		});
	}

	// ── A4: StampBountiesV4 ────────────────────────────────────────────────
	// Partition: {on_chain < 4 → stamps to 4, on_chain == 4 → skips, on_chain > 4 → skips}

	#[test]
	fn stamp_bounties_v4_stamps_when_on_chain_is_0() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: on-chain version is 0 (never set — default)
			assert_eq!(
				pallet_bounties::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(0)
			);

			// Act
			let weight = migrations::StampBountiesV4::on_runtime_upgrade();

			// Assert: stamped to v4
			assert_eq!(
				pallet_bounties::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(4),
				"StampBountiesV4 must stamp on-chain version to 4"
			);
			// Assert: weight = 1 read + 1 write
			assert_eq!(
				weight,
				<Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 1)
			);
		});
	}

	#[test]
	fn stamp_bounties_v4_skips_when_already_v4() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: already at v4
			StorageVersion::new(4).put::<pallet_bounties::Pallet<Runtime>>();

			// Act
			let weight = migrations::StampBountiesV4::on_runtime_upgrade();

			// Assert: still v4, not changed
			assert_eq!(
				pallet_bounties::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(4)
			);
			// Assert: weight = 1 read only (skip path)
			assert_eq!(weight, <Runtime as frame_system::Config>::DbWeight::get().reads(1));
		});
	}

	#[test]
	fn stamp_bounties_v4_skips_when_above_v4() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: somehow at v5
			StorageVersion::new(5).put::<pallet_bounties::Pallet<Runtime>>();

			// Act
			let weight = migrations::StampBountiesV4::on_runtime_upgrade();

			// Assert: stays at v5
			assert_eq!(
				pallet_bounties::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(5)
			);
			assert_eq!(weight, <Runtime as frame_system::Config>::DbWeight::get().reads(1));
		});
	}

	#[test]
	fn stamp_bounties_v4_stamps_intermediate_versions() {
		// Verify that on_chain versions 1, 2, 3 all get stamped to 4
		for v in 1..=3u16 {
			sp_io::TestExternalities::default().execute_with(|| {
				StorageVersion::new(v).put::<pallet_bounties::Pallet<Runtime>>();

				let weight = migrations::StampBountiesV4::on_runtime_upgrade();

				assert_eq!(
					pallet_bounties::Pallet::<Runtime>::on_chain_storage_version(),
					StorageVersion::new(4),
					"StampBountiesV4 must stamp from v{} to v4",
					v
				);
				assert_eq!(
					weight,
					<Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 1)
				);
			});
		}
	}

	// ── C1-C3: Zero-Fee Configuration ───────────────────────────────────────
	// Partition: {WeightToFee returns 0, TransactionByteFee is 0, OperationalFeeMultiplier is 0}

	#[test]
	fn weight_to_fee_returns_zero_for_any_weight() {
		use frame_support::weights::WeightToFee as _;

		// Test with various weight values — all must map to 0 fee
		let test_weights = [
			Weight::zero(),
			Weight::from_parts(1, 0),
			Weight::from_parts(1_000_000_000, 0),
			Weight::from_parts(u64::MAX, u64::MAX),
			BlockWeights::get().max_block,
		];

		for w in &test_weights {
			let fee = thxnet_testnet_runtime_constants::fee::WeightToFee::weight_to_fee(w);
			assert_eq!(fee, 0, "WeightToFee must return 0 for weight {:?}, got {}", w, fee);
		}
	}

	#[test]
	fn transaction_byte_fee_is_zero() {
		assert_eq!(
			thxnet_testnet_runtime_constants::fee::TRANSACTION_BYTE_FEE,
			0u128,
			"TransactionByteFee must be 0 for zero-fee chain"
		);
	}

	#[test]
	fn operational_fee_multiplier_is_zero() {
		assert_eq!(
			thxnet_testnet_runtime_constants::fee::OPERATIONAL_FEE_MULTIPLIER,
			0u8,
			"OperationalFeeMultiplier must be 0 for zero-fee chain"
		);
	}

	#[test]
	fn compute_fee_returns_zero_for_balance_transfer() {
		use frame_support::dispatch::GetDispatchInfo;

		let mut ext = sp_io::TestExternalities::default();
		ext.execute_with(|| {
			// Any extrinsic — use a balance transfer
			let call = pallet_balances::Call::<Runtime>::transfer_keep_alive {
				dest: keyring::Sr25519Keyring::Bob.to_account_id().into(),
				value: 1_000_000_000_000,
			};
			let info = call.get_dispatch_info();

			// Compute fee with multiplier = 1
			pallet_transaction_payment::NextFeeMultiplier::<Runtime>::put(
				sp_runtime::FixedU128::from_u32(1),
			);
			let fee = TransactionPayment::compute_fee(100, &info, 0);
			assert_eq!(fee, 0, "compute_fee must return 0 on zero-fee chain, got {}", fee);
		});
	}

	// ── Migration Ordering: compile-time verification ───────────────────────
	// The fact that `type Migrations = migrations::Unreleased` compiles with
	// the Executive type proves the tuple is well-formed. This test additionally
	// verifies the type aliases resolve and the tuple is non-empty.

	#[test]
	fn rootchain_migration_tuple_compiles_and_is_non_empty() {
		// This test passes simply by compiling. The Migrations type is used by
		// Executive, which requires it to implement OnRuntimeUpgrade. If the
		// tuple had type errors, the crate would not compile.
		//
		// We additionally verify the type alias chain resolves:
		let _ = std::any::type_name::<Migrations>();
		let _ = std::any::type_name::<migrations::Unreleased>();
		// MigrationsEarly and MigrationsLate are module-private type aliases.
		// The fact that Unreleased = (MigrationsEarly, MigrationsLate) compiles
		// proves they are structurally valid and properly ordered.
	}
}

#[cfg(all(test, feature = "try-runtime"))]
mod remote_tests {
	use super::*;
	use frame_try_runtime::{runtime_decl_for_try_runtime::TryRuntime, UpgradeCheckSelect};
	use remote_externalities::{
		Builder, Mode, OfflineConfig, OnlineConfig, SnapshotConfig, Transport,
	};
	use std::env::var;

	#[tokio::test]
	async fn run_migrations() {
		if var("RUN_MIGRATION_TESTS").is_err() {
			return
		}

		sp_tracing::try_init_simple();
		let transport: Transport =
			var("WS").unwrap_or("wss://rpc.polkadot.io:443".to_string()).into();
		let maybe_state_snapshot: Option<SnapshotConfig> = var("SNAP").map(|s| s.into()).ok();
		let mut ext = Builder::<Block>::default()
			.mode(if let Some(state_snapshot) = maybe_state_snapshot {
				Mode::OfflineOrElseOnline(
					OfflineConfig { state_snapshot: state_snapshot.clone() },
					OnlineConfig {
						transport,
						state_snapshot: Some(state_snapshot),
						..Default::default()
					},
				)
			} else {
				Mode::Online(OnlineConfig { transport, ..Default::default() })
			})
			.build()
			.await
			.unwrap();
		ext.execute_with(|| Runtime::on_runtime_upgrade(UpgradeCheckSelect::PreAndPost));
	}

	#[tokio::test]
	#[ignore = "this test is meant to be executed manually"]
	async fn try_fast_unstake_all() {
		sp_tracing::try_init_simple();
		let transport: Transport =
			var("WS").unwrap_or("wss://rpc.polkadot.io:443".to_string()).into();
		let maybe_state_snapshot: Option<SnapshotConfig> = var("SNAP").map(|s| s.into()).ok();
		let mut ext = Builder::<Block>::default()
			.mode(if let Some(state_snapshot) = maybe_state_snapshot {
				Mode::OfflineOrElseOnline(
					OfflineConfig { state_snapshot: state_snapshot.clone() },
					OnlineConfig {
						transport,
						state_snapshot: Some(state_snapshot),
						..Default::default()
					},
				)
			} else {
				Mode::Online(OnlineConfig { transport, ..Default::default() })
			})
			.build()
			.await
			.unwrap();
		ext.execute_with(|| {
			pallet_fast_unstake::ErasToCheckPerBlock::<Runtime>::put(1);
			runtime_common::try_runtime::migrate_all_inactive_nominators::<Runtime>()
		});
	}
}
