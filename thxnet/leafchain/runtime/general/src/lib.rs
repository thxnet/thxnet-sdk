#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

mod weights;
pub mod xcm_config;

// For Proxy Pallet
use codec::{Decode, Encode, MaxEncodedLen};
use cumulus_pallet_parachain_system::RelayNumberStrictlyIncreases;
use cumulus_primitives_core::{AggregateMessageOrigin, ParaId};
use frame_support::{
	construct_runtime,
	dispatch::DispatchClass,
	parameter_types,
	traits::{
		tokens::nonfungibles_v2::Inspect, AsEnsureOriginWithArg, ConstBool, ConstU128, ConstU16,
		ConstU32, ConstU64, Everything, InstanceFilter, TransformOrigin,
	},
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, ConstantMultiplier, Weight},
	PalletId,
};
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureRoot, EnsureSigned,
};
use pallet_nfts::PalletFeatures;
use parachains_common::message_queue::{NarrowOriginToSibling, ParaIdToSibling};
use polkadot_runtime_common::{BlockHashCount, SlowAdjustingFeeUpdate};
use sp_api::impl_runtime_apis;
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{
		AccountIdConversion, AccountIdLookup, BlakeTwo256, Block as BlockT, ConvertInto,
		IdentifyAccount, Verify,
	},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, MultiSignature,
};
pub use sp_runtime::{MultiAddress, Perbill, Permill};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use weights::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight};
// XCM Imports
use xcm::latest::prelude::BodyId;
use xcm_config::XcmOriginToTransactDispatchOrigin;

/// Runtime API definition for assets.
pub mod assets_api;

/// Constant values used within the runtime.
pub mod constants;
pub use constants::{currency::*, fee::*, time::*};

/// Implementations of some helper traits passed into runtime modules as
/// associated types.
pub mod impls;
use impls::{CreditToBlockAuthor, CrowdfundingLifecycleGuard, RwaLicenseVerifier};

/// Alias to 512-bit hash when used in the context of a transaction signature on
/// the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it
/// equivalent to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Balance of an account.
pub type Balance = u128;

/// Nonce of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// An index to a block.
pub type BlockNumber = u32;

/// The address format for describing accounts.
pub type Address = MultiAddress<AccountId, ()>;

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;

/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;

/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_asset_tx_payment::ChargeAssetTxPayment<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;

/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra>;

/// Force-stamp cumulus_pallet_dmp_queue StorageVersion to v2.
///
/// Context: All 9 leafchains have DmpQueue with ZERO storage keys (no data).
/// On-chain StorageVersion is NULL (never set, reads as 0). Code declares STORAGE_VERSION = 2.
/// The lazy migration stub in v1.12.0 DmpQueue does NOT write a version stamp itself.
/// Without this stamp, try-runtime fails on `on_chain != in_code` assertion.
///
/// Safety: No data exists — purely a metadata correction.
pub struct InitDmpQueueStorageVersion;
impl frame_support::traits::OnRuntimeUpgrade for InitDmpQueueStorageVersion {
	fn on_runtime_upgrade() -> Weight {
		use frame_support::traits::GetStorageVersion;
		let on_chain = cumulus_pallet_dmp_queue::Pallet::<Runtime>::on_chain_storage_version();
		if on_chain < 2 {
			log::info!(
				target: "runtime::dmp_queue",
				"InitDmpQueueStorageVersion: stamping on-chain version from {:?} to 2",
				on_chain,
			);
			frame_support::traits::StorageVersion::new(2)
				.put::<cumulus_pallet_dmp_queue::Pallet<Runtime>>();
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 1)
		} else {
			log::info!(
				target: "runtime::dmp_queue",
				"InitDmpQueueStorageVersion: already at {:?}, skipping",
				on_chain,
			);
			<Runtime as frame_system::Config>::DbWeight::get().reads(1)
		}
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		use frame_support::traits::GetStorageVersion;
		frame_support::ensure!(
			cumulus_pallet_dmp_queue::Pallet::<Runtime>::on_chain_storage_version() >= 2,
			"DmpQueue on-chain version should be >= 2 after migration"
		);
		Ok(())
	}
}

/// Force-stamp pallet_crowdfunding StorageVersion to v3.
///
/// Context: The upstream MigrateToV3 guard checks `on_chain == 2` before migrating.
/// On ALL leafchains, on-chain is 0 (never set), so the guard always skips, leaving
/// on_chain stuck at 0 while code declares v3 — causing try-runtime assertion failure.
///
/// Chain-specific scenarios:
/// - Avatect mainnet: on-chain v0, but data IS already v3 format (protocol_fee_bps exists in all 21
///   campaigns from genesis). MigrateToV3 would skip due to guard. Stamp is safe — no data
///   transformation needed.
/// - All other chains: on-chain v0, ZERO Crowdfunding data. Stamp is trivially safe.
/// - Sand testnet: Crowdfunding pallet was never deployed. On-chain reads as v0. Stamp is trivially
///   safe.
/// Clear stale ParachainSystem::HostConfiguration.
///
/// v0.9.40-era cumulus stored `AbridgedHostConfiguration` WITHOUT `async_backing_params`
/// (9 fields). v1.12.0 cumulus adds `async_backing_params` (10 fields). Post-setCode,
/// the new runtime tries to decode old-format bytes → "Not enough data to fill buffer"
/// → block production halts. Every parachain block re-populates this storage from the
/// relay inherent, so killing the stored value is safe — the next block writes fresh
/// bytes in the new format.
pub struct ClearStaleHostConfiguration;
impl frame_support::traits::OnRuntimeUpgrade for ClearStaleHostConfiguration {
	fn on_runtime_upgrade() -> Weight {
		// ParachainSystem::HostConfiguration = twox128("ParachainSystem") + twox128("HostConfiguration")
		// Hardcoded since hex-literal is behind runtime-benchmarks feature flag.
		let key: [u8; 32] = [
			0x45, 0x32, 0x3d, 0xf7, 0xcc, 0x47, 0x15, 0x0b,
			0x39, 0x30, 0xe2, 0x66, 0x6b, 0x0a, 0xa3, 0x13,
			0xc5, 0x22, 0x23, 0x18, 0x80, 0x23, 0x8a, 0x0c,
			0x56, 0x02, 0x1b, 0x87, 0x44, 0xa0, 0x07, 0x43,
		];
		frame_support::storage::unhashed::kill(&key);
		log::info!(
			target: "runtime::parachain_system",
			"ClearStaleHostConfiguration: killed stale AbridgedHostConfiguration; \
			 next block will re-populate from relay inherent with new format",
		);
		<Runtime as frame_system::Config>::DbWeight::get().writes(1)
	}
}

pub struct CrowdfundingStampOrMigrateToV3;
impl frame_support::traits::OnRuntimeUpgrade for CrowdfundingStampOrMigrateToV3 {
	fn on_runtime_upgrade() -> Weight {
		use frame_support::traits::GetStorageVersion;
		let on_chain = pallet_crowdfunding::Pallet::<Runtime>::on_chain_storage_version();
		if on_chain < 3 {
			log::info!(
				target: "runtime::crowdfunding",
				"CrowdfundingStampOrMigrateToV3: stamping on-chain version from {:?} to 3 \
				(data already in v3 format or non-existent)",
				on_chain,
			);
			frame_support::traits::StorageVersion::new(3)
				.put::<pallet_crowdfunding::Pallet<Runtime>>();
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 1)
		} else {
			log::info!(
				target: "runtime::crowdfunding",
				"CrowdfundingStampOrMigrateToV3: already at {:?}, skipping",
				on_chain,
			);
			<Runtime as frame_system::Config>::DbWeight::get().reads(1)
		}
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		use frame_support::traits::GetStorageVersion;
		frame_support::ensure!(
			pallet_crowdfunding::Pallet::<Runtime>::on_chain_storage_version() >= 3,
			"Crowdfunding on-chain version should be >= 3 after migration"
		);
		Ok(())
	}
}

// Leafchains have only 4-6 identity entries each; u64::MAX is safe here because
// the migration iterates all entries in a single block, and the VersionedMigration
// wrapper ensures it runs at most once (gated on on-chain StorageVersion == 0).
// With so few entries the PoV / weight impact is negligible.
const IDENTITY_MIGRATION_KEY_LIMIT: u64 = u64::MAX;

/// Cumulative migrations for live leafchains upgrading from v0.9.40 to v1.12.0.
///
/// On-chain state at v0.9.40:
///   - XcmpQueue: v2 (ECQ chains) or v3 (Group A chains)
///   - DmpQueue:  v0 (NULL, never set) — all chains have ZERO DmpQueue storage keys
///   - CollatorSelection: v0
///   - Rwa:            v0 (new pallet, or v5 on Avatect)
///   - Crowdfunding:   v0 (never set, but data is v3 format on Avatect)
///   - TrustlessAgent: v0 (new pallet)
///   - Treasury:       v0 (NULL, in on-chain metadata but not in old source)
///
/// Each migration is version-guarded internally (VersionedMigration or manual check).
/// Including all steps is safe — guards auto-skip when on-chain version doesn't match.
pub type Migrations = (
	// Clear stale ParachainSystem::HostConfiguration (must run FIRST before any
	// block post-upgrade tries to decode the old-format bytes).
	ClearStaleHostConfiguration,
	// ── Frame / Cumulus pallet migrations ──
	// CollatorSelection: invulnerable storage format change (v0→v1)
	pallet_collator_selection::migration::v1::MigrateToV1<Runtime>,
	// XcmpQueue: QueueConfigData 1D Weight → 2D Weight (v1→v2)
	cumulus_pallet_xcmp_queue::migration::v2::MigrationToV2<Runtime>,
	// XcmpQueue: Overweight counter initialization (v2→v3)
	cumulus_pallet_xcmp_queue::migration::v3::MigrationToV3<Runtime>,
	// XcmpQueue: QueueConfigData simplification, drop deprecated fields (v3→v4)
	cumulus_pallet_xcmp_queue::migration::v4::MigrationToV4<Runtime>,
	// XCM pallet: migrate stored XCM versions to latest (covers v3→v4 transition)
	pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>,
	// CollatorSelection v1→v2 (Candidates → CandidateList)
	pallet_collator_selection::migration::v2::MigrationToV2<Runtime>,
	// DmpQueue: force-stamp StorageVersion to v2 (all chains have 0 DmpQueue data)
	InitDmpQueueStorageVersion,
	// Identity: username/authority feature migration (v0→v1)
	pallet_identity::migration::versioned::V0ToV1<Runtime, IDENTITY_MIGRATION_KEY_LIMIT>,
	// ── Custom pallet migrations ──
	// RWA: stamp on-chain version to v5 (noop if already ≥5, no data change)
	pallet_rwa::migrations::v5::MigrateToV5<Runtime>,
	// Crowdfunding: force-stamp to v3 (replaces MigrateToV3 which has broken v2 guard)
	CrowdfundingStampOrMigrateToV3,
	// TrustlessAgent: initial deployment, initialize counters (v0→v1)
	pallet_trustless_agent::migrations::Migrations<Runtime>,
);

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
	Migrations,
>;

/// Opaque types. These are used by the CLI to instantiate machinery that don't
/// need to know the specifics of the runtime. They can then be made to be
/// agnostic over specific formats of data like extrinsics, allowing for them to
/// continue syncing the network through upgrades to even the core data
/// structures.
pub mod opaque {
	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;
	use sp_runtime::{generic, traits::BlakeTwo256};

	use super::*;
	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;
}

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
	}
}

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("thxnet-general-runtime"),
	impl_name: create_runtime_str!("thxnet-general-runtime"),
	authoring_version: 1,
	spec_version: 21,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	state_version: 1,
};

/// We assume that ~5% of the block weight is consumed by `on_initialize`
/// handlers. This is used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);

/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be
/// used by `Operational` extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// We allow for 0.5 of a second of compute with a 12 second average block time.
const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(
	WEIGHT_REF_TIME_PER_SECOND.saturating_div(2),
	cumulus_primitives_core::relay_chain::MAX_POV_SIZE as u64,
);

/// Maximum number of blocks simultaneously accepted by the Runtime, not yet included
/// into the relay chain.
///
/// v1.12.0 WORKAROUND: set to 1 to prevent cumulus fork production at the source.
/// The v1.12.0 relay-side prospective-parachains subsystem uses the pre-#4937
/// fragment-chain (`fragment_chain/mod.rs:797`'s `is_fork_or_cycle` rejects any
/// second candidate at the same parent). In small/uniform topologies (1 core, 1
/// backing group) the collator re-authors block N every slot until inclusion,
/// producing forks; fragment-chain rejects them all as "Is not a potential
/// member", inclusion never completes, para stalls permanently.
///
/// Capacity=1 forces `cumulus_pallet_aura_ext::FixedVelocityConsensusHook::
/// can_build_upon` to return false while any block is unincluded, so the collator
/// builds exactly one block per inclusion cycle. No forks → fragment-chain
/// accepts every candidate → inclusion pipeline stays healthy. Tradeoff: para
/// throughput is synchronous-backing tempo (~18s per block instead of async's
/// ~6s), but stable.
///
/// Empirically validated on forked-testnet 2026-04-18: with
/// `BLOCK_PROCESSING_VELOCITY=1, UNINCLUDED_SEGMENT_CAPACITY=1`, para reached
/// block 4556+ with finalization keeping pace. With capacity=2 under the same
/// topology, para stalls at ~13-30 forever.
///
/// The stable2512 hop should restore capacity to 2 (async backing safe once
/// #4937 is present).
const UNINCLUDED_SEGMENT_CAPACITY: u32 = 1;
/// How many parachain blocks are processed by the relay chain per parent. Limits the
/// number of blocks authored per slot.
const BLOCK_PROCESSING_VELOCITY: u32 = 1;
/// Relay chain slot duration, in milliseconds.
const RELAY_CHAIN_SLOT_DURATION_MILLIS: u32 = 6000;

/// The version information used to identify this runtime when compiled
/// natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

parameter_types! {
	pub const Version: RuntimeVersion = VERSION;

	// This part is copied from Substrate's `bin/node/runtime/src/lib.rs`.
	//  The `RuntimeBlockLength` and `RuntimeBlockWeights` exist here because the
	// `DeletionWeightLimit` and `DeletionQueueDepth` depend on those to parameterize
	// the lazy contract deletion.
	pub RuntimeBlockLength: BlockLength =
		BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have some extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
	pub const SS58Prefix: u16 = 42;
}

// Configure FRAME pallets to include in runtime.

impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type RuntimeCall = RuntimeCall;
	type RuntimeTask = RuntimeTask;
	type Lookup = AccountIdLookup<AccountId, ()>;
	type Nonce = Nonce;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type BlockHashCount = BlockHashCount;
	type Version = Version;
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = RocksDbWeight;
	type BaseCallFilter = Everything;
	type SystemWeightInfo = ();
	type BlockWeights = RuntimeBlockWeights;
	type BlockLength = RuntimeBlockLength;
	type SS58Prefix = SS58Prefix;
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
}

impl pallet_timestamp::Config for Runtime {
	type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Aura;
	type WeightInfo = ();
}

impl pallet_authorship::Config for Runtime {
	type EventHandler = (CollatorSelection,);
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
}

parameter_types! {
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}

impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	/// The type for recording an account's balance.
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type FreezeIdentifier = ();
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type MaxFreezes = ConstU32<0>;
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const TransactionByteFee: Balance = TRANSACTION_BYTE_FEE;
	pub const OperationalFeeMultiplier: u8 = OPERATIONAL_FEE_MULTIPLIER;
}

impl pallet_transaction_payment::Config for Runtime {
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type OnChargeTransaction = pallet_transaction_payment::CurrencyAdapter<Balances, ()>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type RuntimeEvent = RuntimeEvent;
	type WeightToFee = WeightToFee;
}

impl pallet_asset_tx_payment::Config for Runtime {
	type Fungibles = Assets;
	type OnChargeAssetTransaction = pallet_asset_tx_payment::FungiblesAdapter<
		pallet_assets::BalanceToAssetBalance<Balances, Runtime, ConvertInto>,
		CreditToBlockAuthor,
	>;
	type RuntimeEvent = RuntimeEvent;
}

parameter_types! {
	pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
	pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
}

impl cumulus_pallet_parachain_system::Config for Runtime {
	type CheckAssociatedRelayNumber = RelayNumberStrictlyIncreases;
	type ConsensusHook = cumulus_pallet_aura_ext::FixedVelocityConsensusHook<
		Runtime,
		RELAY_CHAIN_SLOT_DURATION_MILLIS,
		BLOCK_PROCESSING_VELOCITY,
		UNINCLUDED_SEGMENT_CAPACITY,
	>;
	type DmpQueue = frame_support::traits::EnqueueWithOrigin<MessageQueue, RelayOrigin>;
	type OnSystemEvent = ();
	type OutboundXcmpMessageSource = XcmpQueue;
	type ReservedDmpWeight = ReservedDmpWeight;
	type ReservedXcmpWeight = ReservedXcmpWeight;
	type RuntimeEvent = RuntimeEvent;
	type SelfParaId = parachain_info::Pallet<Runtime>;
	type WeightInfo = ();
	type XcmpMessageHandler = XcmpQueue;
}

impl parachain_info::Config for Runtime {}

impl cumulus_pallet_aura_ext::Config for Runtime {}

parameter_types! {
	pub MessageQueueServiceWeight: Weight = Perbill::from_percent(35) * RuntimeBlockWeights::get().max_block;
	pub const RelayOrigin: AggregateMessageOrigin = AggregateMessageOrigin::Parent;
}

impl pallet_message_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type MessageProcessor = xcm_builder::ProcessXcmMessage<
		AggregateMessageOrigin,
		xcm_executor::XcmExecutor<xcm_config::XcmConfig>,
		RuntimeCall,
	>;
	type Size = u32;
	type QueueChangeHandler = NarrowOriginToSibling<XcmpQueue>;
	type QueuePausedQuery = NarrowOriginToSibling<XcmpQueue>;
	type HeapSize = sp_core::ConstU32<{ 64 * 1024 }>;
	type MaxStale = sp_core::ConstU32<8>;
	type ServiceWeight = MessageQueueServiceWeight;
	type IdleMaxServiceWeight = MessageQueueServiceWeight;
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = ();
	type XcmpQueue = TransformOrigin<MessageQueue, AggregateMessageOrigin, ParaId, ParaIdToSibling>;
	type MaxInboundSuspended = sp_core::ConstU32<1_000>;
	type ControllerOrigin = EnsureRoot<AccountId>;
	type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
	type PriceForSiblingDelivery = polkadot_runtime_common::xcm_sender::NoPriceForMessageDelivery<
		cumulus_primitives_core::ParaId,
	>;
	type WeightInfo = ();
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type DmpSink = frame_support::traits::EnqueueWithOrigin<MessageQueue, RelayOrigin>;
	type WeightInfo = ();
}

parameter_types! {
	pub const Period: u32 = 6 * HOURS;
	pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
	type Keys = SessionKeys;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type RuntimeEvent = RuntimeEvent;
	// Essentially just Aura, but let's be pedantic.
	type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
	type SessionManager = CollatorSelection;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type WeightInfo = ();
}

impl pallet_aura::Config for Runtime {
	type AllowMultipleBlocksPerSlot = ConstBool<false>;
	type AuthorityId = AuraId;
	type DisabledValidators = ();
	type MaxAuthorities = ConstU32<100_000>;
	type SlotDuration = ConstU64<SLOT_DURATION>;
}

parameter_types! {
	pub const PotId: PalletId = PalletId(*b"PotStake");
	pub const MaxCandidates: u32 = 1000;
	pub const SessionLength: BlockNumber = 6 * HOURS;
	pub const MaxInvulnerables: u32 = 100;
	pub const ExecutiveBody: BodyId = BodyId::Executive;
}

// We allow root only to execute privileged collator selection operations.
pub type CollatorSelectionUpdateOrigin = EnsureRoot<AccountId>;

impl pallet_collator_selection::Config for Runtime {
	type Currency = Balances;
	// should be a multiple of session or things will get inconsistent
	type KickThreshold = Period;
	type MaxCandidates = MaxCandidates;
	type MaxInvulnerables = MaxInvulnerables;
	type MinEligibleCollators = ConstU32<4>;
	type PotId = PotId;
	type RuntimeEvent = RuntimeEvent;
	type UpdateOrigin = CollatorSelectionUpdateOrigin;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ValidatorRegistration = Session;
	type WeightInfo = ();
}

impl pallet_sudo::Config for Runtime {
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

// ── Treasury Pallet ─────────────────────────────────────────────────────
//
// Treasury was present in on-chain metadata (index 19) for all 9 leafchains
// at genesis, with NULL StorageVersion and 0 data keys. It was later removed
// from the source code but stayed in on-chain state. Re-adding it here to
// keep the runtime consistent with on-chain metadata.

parameter_types! {
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 100 * DOLLARS;
	pub const ProposalBondMaximum: Balance = 500 * DOLLARS;
	pub const SpendPeriod: BlockNumber = 24 * DAYS;
	pub const TreasuryBurn: Permill = Permill::from_percent(1);
	pub const MaxApprovals: u32 = 100;
}

impl pallet_treasury::Config for Runtime {
	type PalletId = TreasuryPalletId;
	type Currency = Balances;
	type ApproveOrigin = EnsureRoot<AccountId>;
	type RejectOrigin = EnsureRoot<AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type OnSlash = Treasury;
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type ProposalBondMaximum = ProposalBondMaximum;
	type SpendPeriod = SpendPeriod;
	type Burn = TreasuryBurn;
	type BurnDestination = ();
	type SpendFunds = ();
	type MaxApprovals = MaxApprovals;
	type WeightInfo = pallet_treasury::weights::SubstrateWeight<Runtime>;
	type SpendOrigin = frame_support::traits::NeverEnsureOrigin<Balance>;
	type AssetKind = ();
	type Beneficiary = AccountId;
	type BeneficiaryLookup = sp_runtime::traits::IdentityLookup<AccountId>;
	type Paymaster = frame_support::traits::tokens::PayFromAccount<Balances, TreasuryAccount>;
	type BalanceConverter = frame_support::traits::tokens::UnityAssetBalanceConversion;
	type PayoutPeriod = SpendPeriod;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

parameter_types! {
	// One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
	pub const DepositBase: Balance = deposit(1, 88);
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = deposit(0, 32);
	pub const MaxSignatories: u32 = 100;
}

impl pallet_multisig::Config for Runtime {
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_multisig::weights::SubstrateWeight<Runtime>;
}

impl pallet_utility::Config for Runtime {
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const AssetDeposit: Balance = 100 * UNITS;
	pub const ApprovalDeposit: Balance = 1 * UNITS;
	pub const StringLimit: u32 = 50;
	pub const MetadataDepositBase: Balance = 10 * UNITS;
	pub const MetadataDepositPerByte: Balance = 1 * UNITS;
}

impl pallet_assets::Config for Runtime {
	type ApprovalDeposit = ApprovalDeposit;
	type AssetAccountDeposit = ConstU128<UNITS>;
	type AssetDeposit = AssetDeposit;
	type AssetId = u32;
	type AssetIdParameter = codec::Compact<u32>;
	type Balance = u128;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
	type CallbackHandle = ();
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type Currency = Balances;
	type Extra = ();
	type ForceOrigin = EnsureRoot<AccountId>;
	type Freezer = ();
	type MetadataDepositBase = MetadataDepositBase;
	type MetadataDepositPerByte = MetadataDepositPerByte;
	type RemoveItemsLimit = ConstU32<1000>;
	type RuntimeEvent = RuntimeEvent;
	type StringLimit = StringLimit;
	type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub Features: PalletFeatures = PalletFeatures::all_enabled();
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
	type ApprovalsLimit = ApprovalsLimit;
	type AttributeDepositBase = MetadataDepositBase;
	type CollectionDeposit = CollectionDeposit;
	type CollectionId = u32;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type Currency = Balances;
	type DepositPerByte = MetadataDepositPerByte;
	type Features = Features;
	type ForceOrigin = frame_system::EnsureRoot<AccountId>;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
	type ItemAttributesApprovalsLimit = ItemAttributesApprovalsLimit;
	type ItemDeposit = ItemDeposit;
	type ItemId = u32;
	type KeyLimit = KeyLimit;
	type Locker = ();
	type MaxAttributesPerCall = MaxAttributesPerCall;
	type MaxDeadlineDuration = MaxDeadlineDuration;
	type MaxTips = MaxTips;
	type MetadataDepositBase = MetadataDepositBase;
	type OffchainPublic = <Signature as sp_runtime::traits::Verify>::Signer;
	type OffchainSignature = Signature;
	type RuntimeEvent = RuntimeEvent;
	type StringLimit = StringLimit;
	type ValueLimit = ValueLimit;
	type WeightInfo = pallet_nfts::weights::SubstrateWeight<Runtime>;
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
	sp_runtime::RuntimeDebug,
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
	// Auction = 7,
}

#[cfg(test)]
mod proxy_type_tests {
	use super::*;

	#[derive(
		Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, sp_runtime::RuntimeDebug,
	)]
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

// ════════════════════════════════════════════════════════════════════════════
// Migration correctness tests for leafchain v0.9.40 → v1.12.0 upgrade.
//
// These tests validate every custom leafchain migration that has NO existing
// unit tests. Each test targets exactly one MECE partition.
// ════════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod leafchain_migration_tests {
	use super::*;
	use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};

	// ── A4: InitDmpQueueStorageVersion ──────────────────────────────────────
	// Partition: {on_chain < 2 → stamps to 2, on_chain >= 2 → skips}

	#[test]
	fn init_dmp_queue_stamps_v2_when_on_chain_is_0() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: on-chain version is 0 (never set — default)
			assert_eq!(
				cumulus_pallet_dmp_queue::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(0)
			);

			// Act
			let weight = InitDmpQueueStorageVersion::on_runtime_upgrade();

			// Assert: stamped to v2
			assert_eq!(
				cumulus_pallet_dmp_queue::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(2),
				"InitDmpQueueStorageVersion must stamp on-chain version to 2"
			);
			// Assert: weight = 1 read + 1 write
			assert_eq!(
				weight,
				<Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 1)
			);
		});
	}

	#[test]
	fn init_dmp_queue_skips_when_already_v2() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: already at v2
			StorageVersion::new(2).put::<cumulus_pallet_dmp_queue::Pallet<Runtime>>();

			// Act
			let weight = InitDmpQueueStorageVersion::on_runtime_upgrade();

			// Assert: still v2, not changed
			assert_eq!(
				cumulus_pallet_dmp_queue::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(2)
			);
			// Assert: weight = 1 read only (skip path)
			assert_eq!(weight, <Runtime as frame_system::Config>::DbWeight::get().reads(1));
		});
	}

	#[test]
	fn init_dmp_queue_skips_when_above_v2() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: somehow at v3
			StorageVersion::new(3).put::<cumulus_pallet_dmp_queue::Pallet<Runtime>>();

			// Act
			let weight = InitDmpQueueStorageVersion::on_runtime_upgrade();

			// Assert: stays at v3
			assert_eq!(
				cumulus_pallet_dmp_queue::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(3)
			);
			assert_eq!(weight, <Runtime as frame_system::Config>::DbWeight::get().reads(1));
		});
	}

	// ── A5: CrowdfundingStampOrMigrateToV3 ──────────────────────────────────
	// Partition: {on_chain < 3 → stamps to 3, on_chain >= 3 → skips}

	#[test]
	fn crowdfunding_stamp_sets_v3_when_on_chain_is_0() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: on-chain version is 0 (never set)
			assert_eq!(
				pallet_crowdfunding::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(0)
			);

			// Act
			let weight = CrowdfundingStampOrMigrateToV3::on_runtime_upgrade();

			// Assert: stamped to v3
			assert_eq!(
				pallet_crowdfunding::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(3),
				"CrowdfundingStampOrMigrateToV3 must stamp on-chain version to 3"
			);
			assert_eq!(
				weight,
				<Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 1)
			);
		});
	}

	#[test]
	fn crowdfunding_stamp_skips_when_already_v3() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: already at v3
			StorageVersion::new(3).put::<pallet_crowdfunding::Pallet<Runtime>>();

			// Act
			let weight = CrowdfundingStampOrMigrateToV3::on_runtime_upgrade();

			// Assert: still v3
			assert_eq!(
				pallet_crowdfunding::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(3)
			);
			assert_eq!(weight, <Runtime as frame_system::Config>::DbWeight::get().reads(1));
		});
	}

	#[test]
	fn crowdfunding_stamp_sets_v3_when_on_chain_is_2() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: on-chain version is 2 (the "normal" pre-migration state for
			// chains that actually ran v2 migration)
			StorageVersion::new(2).put::<pallet_crowdfunding::Pallet<Runtime>>();

			// Act
			let weight = CrowdfundingStampOrMigrateToV3::on_runtime_upgrade();

			// Assert: stamped to v3 (stamp path, not MigrateToV3 data migration)
			assert_eq!(
				pallet_crowdfunding::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(3),
				"CrowdfundingStampOrMigrateToV3 must stamp from v2 to v3"
			);
			assert_eq!(
				weight,
				<Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 1)
			);
		});
	}

	// ── A6: RWA MigrateToV5 ────────────────────────────────────────────────
	// Partition: {on_chain < 5 → stamps to 5, on_chain >= 5 → skips}

	#[test]
	fn rwa_stamp_sets_v5_when_on_chain_is_0() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: on-chain version is 0 (never set)
			assert_eq!(
				pallet_rwa::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(0)
			);

			// Act
			let weight = pallet_rwa::migrations::v5::MigrateToV5::<Runtime>::on_runtime_upgrade();

			// Assert: stamped to v5
			assert_eq!(
				pallet_rwa::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(5),
				"RWA MigrateToV5 must stamp on-chain version to 5"
			);
			// Assert: weight = 1 write
			assert_eq!(weight, <Runtime as frame_system::Config>::DbWeight::get().writes(1));
		});
	}

	#[test]
	fn rwa_stamp_skips_when_already_v5() {
		sp_io::TestExternalities::default().execute_with(|| {
			// Arrange: already at v5
			StorageVersion::new(5).put::<pallet_rwa::Pallet<Runtime>>();

			// Act
			let weight = pallet_rwa::migrations::v5::MigrateToV5::<Runtime>::on_runtime_upgrade();

			// Assert: still v5
			assert_eq!(
				pallet_rwa::Pallet::<Runtime>::on_chain_storage_version(),
				StorageVersion::new(5)
			);
			// Assert: weight = 0 (skip path)
			assert_eq!(weight, frame_support::weights::Weight::zero());
		});
	}

	// ── Zero-Fee Configuration (Leafchain) ──────────────────────────────────

	#[test]
	fn leafchain_weight_to_fee_returns_zero() {
		use frame_support::weights::{Weight, WeightToFee as WeightToFeeT};

		let test_weights =
			[Weight::zero(), Weight::from_parts(1, 0), Weight::from_parts(u64::MAX, u64::MAX)];
		for w in &test_weights {
			let fee = constants::fee::WeightToFee::weight_to_fee(w);
			assert_eq!(fee, 0, "Leafchain WeightToFee must return 0 for {:?}", w);
		}
	}

	#[test]
	fn leafchain_transaction_byte_fee_is_zero() {
		assert_eq!(constants::fee::TRANSACTION_BYTE_FEE, 0u128);
	}

	#[test]
	fn leafchain_operational_fee_multiplier_is_zero() {
		assert_eq!(constants::fee::OPERATIONAL_FEE_MULTIPLIER, 0u8);
	}

	// ── Leafchain migration tuple compiles ──────────────────────────────────

	#[test]
	fn leafchain_migration_tuple_compiles_and_is_non_empty() {
		// This test passes by compilation alone. The Migrations type is used by
		// Executive (which requires OnRuntimeUpgrade). If the tuple had type errors,
		// or any migration had incorrect generic params, this crate would not compile.
		let _ = core::any::type_name::<Migrations>();
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
			ProxyType::NonTransfer =>
				matches!(
					c,
					RuntimeCall::System(..) |
						RuntimeCall::Timestamp(..) |
						RuntimeCall::Session(..) | RuntimeCall::Utility(..) |
						RuntimeCall::Proxy(..) | RuntimeCall::Multisig(..) |
						RuntimeCall::Assets(..) | RuntimeCall::Nfts(..) |
						RuntimeCall::TrustlessAgent(..)
				),
			ProxyType::Governance => matches!(c, RuntimeCall::Utility(..)),
			ProxyType::Staking => {
				matches!(c, RuntimeCall::Session(..) | RuntimeCall::Utility(..))
			},
			ProxyType::IdentityJudgement => matches!(
				c,
				RuntimeCall::Identity(pallet_identity::Call::provide_judgement { .. }) |
					RuntimeCall::Utility(..)
			),
			ProxyType::CancelProxy => {
				matches!(c, RuntimeCall::Proxy(pallet_proxy::Call::reject_announcement { .. }))
			}, /* ProxyType::Auction => matches!(
			    * 	c,
			    * 	RuntimeCall::Auctions(..) |
			    * 		RuntimeCall::Crowdloan(..) |
			    * 		RuntimeCall::Registrar(..) |
			    * 		RuntimeCall::Slots(..)
			    * ), */
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
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
	type CallHasher = BlakeTwo256;
	type Currency = Balances;
	type MaxPending = MaxPending;
	type MaxProxies = MaxProxies;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type ProxyType = ProxyType;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_proxy::WeightInfo<Runtime>;
}

parameter_types! {
	// Identity pallet deposits
	// Aligned with Rootchain economic standards (100 DOLLARS basic deposit)
	// Using deposit() function: items * 15 * CENTS + bytes * 6 * CENTS
	pub const BasicDeposit: Balance = deposit(667, 0);        // ≈ 100.05 DOLLARS
	pub const ByteDeposit: Balance = deposit(0, 0);            // 0 DOLLARS (free additional fields)
	pub const SubAccountDeposit: Balance = deposit(0, 0);     // 0 DOLLARS (free sub-accounts)
	pub const MaxSubAccounts: u32 = 100;
	pub const MaxAdditionalFields: u32 = 100;
	pub const MaxRegistrars: u32 = 20;
}

impl pallet_identity::Config for Runtime {
	type BasicDeposit = BasicDeposit;
	type Currency = Balances;
	type ByteDeposit = ByteDeposit;
	type ForceOrigin = EnsureRoot<AccountId>;
	type IdentityInformation = pallet_identity::legacy::IdentityInfo<MaxAdditionalFields>;
	type MaxRegistrars = MaxRegistrars;
	type MaxSubAccounts = MaxSubAccounts;
	type RegistrarOrigin = EnsureRoot<AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type Slashed = ();
	type SubAccountDeposit = SubAccountDeposit;
	type WeightInfo = pallet_identity::weights::SubstrateWeight<Runtime>;
	// Username types (v1.6.0)
	type OffchainSignature = Signature;
	type SigningPublicKey = <Signature as Verify>::Signer;
	type UsernameAuthorityOrigin = EnsureRoot<AccountId>;
	type PendingUsernameExpiration = ConstU32<{ 7 * DAYS }>;
	type MaxSuffixLength = ConstU32<7>;
	type MaxUsernameLength = ConstU32<32>;
}

parameter_types! {
	// Trustless Agent pallet parameters
	pub const AgentDeposit: Balance = 100 * DOLLARS;
	pub const FeedbackDeposit: Balance = 10 * DOLLARS;
	pub const ValidatorMinStake: Balance = 1000 * DOLLARS;
	pub const ValidationRequestDeposit: Balance = 5 * DOLLARS;
	pub const DisputeDeposit: Balance = 20 * DOLLARS;
	pub const ValidationDeadline: BlockNumber = 7 * DAYS;
	// Escrow auto-complete duration: 7 days
	pub const EscrowAutoCompleteBlocks: BlockNumber = 7 * DAYS;
	// Feedback rate limit: 7 days between feedbacks from same client to same agent
	pub const FeedbackRateLimitBlocks: BlockNumber = 7 * DAYS;
	pub const MaxUriLength: u32 = 512;
	pub const MaxTagLength: u32 = 64;
	pub const MaxTags: u32 = 20;
	pub const MaxMetadataKeyLength: u32 = 128;
	pub const MaxMetadataValueLength: u32 = 512;
	pub const MaxMetadataEntries: u32 = 50;
	pub const MaxResponsesPerFeedback: u32 = 20;
}

impl pallet_trustless_agent::Config for Runtime {
	type AgentDeposit = AgentDeposit;
	type Currency = Balances;
	type DisputeDeposit = DisputeDeposit;
	type DisputeResolverOrigin = EnsureRoot<AccountId>;
	type EscrowAutoCompleteBlocks = EscrowAutoCompleteBlocks;
	type FeedbackDeposit = FeedbackDeposit;
	type FeedbackRateLimitBlocks = FeedbackRateLimitBlocks;
	type MaxMetadataEntries = MaxMetadataEntries;
	type MaxMetadataKeyLength = MaxMetadataKeyLength;
	type MaxMetadataValueLength = MaxMetadataValueLength;
	type MaxResponsesPerFeedback = MaxResponsesPerFeedback;
	type MaxTagLength = MaxTagLength;
	type MaxTags = MaxTags;
	type MaxUriLength = MaxUriLength;
	type RuntimeEvent = RuntimeEvent;
	type ValidationDeadline = ValidationDeadline;
	type ValidationRequestDeposit = ValidationRequestDeposit;
	type ValidatorManagerOrigin = EnsureRoot<AccountId>;
	type ValidatorMinStake = ValidatorMinStake;
	type WeightInfo = weights::pallet_trustless_agent::WeightInfo<Runtime>;
}

// ── RWA Pallet ──────────────────────────────────────────────────────────

parameter_types! {
	// MUST match old leafchains runtime PalletId exactly -- sub-account derivation.
	pub const RwaPalletId: PalletId = PalletId(*b"py/rwaaa");
	pub const RwaAssetRegistrationDeposit: Balance = 100 * DOLLARS;
	pub const RwaMaxAssetsPerOwner: u32 = 100;
	pub const RwaMaxMetadataLen: u32 = 256;
	pub const RwaMaxSlashRecipients: u32 = 5;
	pub const RwaMaxGroupSize: u32 = 10;
	pub const RwaMaxPendingApprovals: u32 = 100;
	// MUST be >= old leafchains value (50) for BoundedVec decode safety.
	pub const RwaMaxSunsettingPerBlock: u32 = 50;
	// Match old leafchains (50). Can increase later but never decrease.
	pub const RwaMaxParticipationsPerHolder: u32 = 50;
	pub const RwaMinParticipationDeposit: Balance = 1 * DOLLARS;
}

impl pallet_rwa::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = u32;
	type NativeCurrency = Balances;
	type Fungibles = Assets;
	type AdminOrigin = EnsureRoot<AccountId>;
	type ForceOrigin = EnsureRoot<AccountId>;
	type PalletId = RwaPalletId;
	type AssetRegistrationDeposit = RwaAssetRegistrationDeposit;
	type MaxAssetsPerOwner = RwaMaxAssetsPerOwner;
	type MaxMetadataLen = RwaMaxMetadataLen;
	type MaxSlashRecipients = RwaMaxSlashRecipients;
	type MaxGroupSize = RwaMaxGroupSize;
	type MaxPendingApprovals = RwaMaxPendingApprovals;
	type MaxSunsettingPerBlock = RwaMaxSunsettingPerBlock;
	type MaxParticipationsPerHolder = RwaMaxParticipationsPerHolder;
	type MinParticipationDeposit = RwaMinParticipationDeposit;
	type WeightInfo = pallet_rwa::weights::SubstrateWeight<Runtime>;
	type ParticipationFilter = ();
	type AssetLifecycleGuard = CrowdfundingLifecycleGuard;
}

// ── Crowdfunding Pallet ─────────────────────────────────────────────────

parameter_types! {
	// MUST match old leafchains runtime PalletId exactly -- sub-account derivation.
	pub const CrowdfundingPalletId: PalletId = PalletId(*b"py/crwdf");
	pub const CfCampaignCreationDeposit: Balance = 50 * DOLLARS;
	// MUST be >= old leafchains value (20) for BoundedVec decode safety.
	pub const CfMaxCampaignsPerCreator: u32 = 20;
	pub const CfMinCampaignDuration: BlockNumber = 1 * DAYS;
	pub const CfMaxCampaignDuration: BlockNumber = 90 * DAYS;
	pub const CfEarlyWithdrawalPenaltyBps: u16 = 100;
	pub const CfMaxMilestones: u32 = 10;
	pub const CfMaxEligibilityRules: u32 = 5;
	pub const CfMaxNftSets: u32 = 5;
	pub const CfMaxNftsPerSet: u32 = 5;
	// MUST be >= old leafchains value (50) for BoundedVec decode safety.
	pub const CfMaxInvestmentsPerInvestor: u32 = 50;
	pub const CfProtocolFeeBps: u16 = 200;
	pub const CfMaxWhitelistSize: u32 = 500;
	// WARNING: This is the burn address [0u8;32]. MUST be reconfigured via
	// set_protocol_config before any campaigns are created on mainnet.
	pub CfProtocolFeeRecipient: AccountId = AccountId::from([0u8; 32]);
}

impl pallet_crowdfunding::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = u32;
	type CollectionId = u32;
	type ItemId = u32;
	type NativeCurrency = Balances;
	type Fungibles = Assets;
	type NftInspect = Nfts;
	type AdminOrigin = EnsureRoot<AccountId>;
	type ForceOrigin = EnsureRoot<AccountId>;
	type MilestoneApprover = EnsureRoot<AccountId>;
	type PalletId = CrowdfundingPalletId;
	type CampaignCreationDeposit = CfCampaignCreationDeposit;
	type MaxCampaignsPerCreator = CfMaxCampaignsPerCreator;
	type MinCampaignDuration = CfMinCampaignDuration;
	type MaxCampaignDuration = CfMaxCampaignDuration;
	type EarlyWithdrawalPenaltyBps = CfEarlyWithdrawalPenaltyBps;
	type MaxMilestones = CfMaxMilestones;
	type MaxEligibilityRules = CfMaxEligibilityRules;
	type MaxNftSets = CfMaxNftSets;
	type MaxNftsPerSet = CfMaxNftsPerSet;
	type MaxInvestmentsPerInvestor = CfMaxInvestmentsPerInvestor;
	type ProtocolFeeBps = ConstU16<200>;
	type ProtocolFeeRecipient = CfProtocolFeeRecipient;
	type MaxWhitelistSize = CfMaxWhitelistSize;
	type LicenseVerifier = RwaLicenseVerifier;
	type WeightInfo = pallet_crowdfunding::weights::SubstrateWeight<Runtime>;
}

// Create the runtime by composing the FRAME pallets that were previously
// configured.
construct_runtime!(
	pub enum Runtime
	{
		// System support stuff.
		System: frame_system = 0,
		ParachainSystem: cumulus_pallet_parachain_system = 1,
		Timestamp: pallet_timestamp = 2,
		ParachainInfo: parachain_info = 3,
		Utility: pallet_utility = 4,

		// Multisig dispatch.
		Multisig: pallet_multisig = 5,

		// Monetary stuff.
		Balances: pallet_balances = 10,
		TransactionPayment: pallet_transaction_payment = 11,
		AssetTxPayment: pallet_asset_tx_payment = 12,
		Assets: pallet_assets = 13,
		Nfts: pallet_nfts = 14,

		// Treasury (present in on-chain metadata at index 19 since genesis, 0 data keys)
		Treasury: pallet_treasury = 19,

		// Collator support. The order of these 4 are important and shall not change.
		Authorship: pallet_authorship = 20,
		CollatorSelection: pallet_collator_selection = 21,
		Session: pallet_session = 22,
		Aura: pallet_aura = 23,
		AuraExt: cumulus_pallet_aura_ext = 24,

		// Trustless Agent
		TrustlessAgent: pallet_trustless_agent = 27,

		// Identity
		Identity: pallet_identity = 28,

		// Proxy
		Proxy: pallet_proxy = 29,

		// XCM helpers.
		XcmpQueue: cumulus_pallet_xcmp_queue = 30,
		PolkadotXcm: pallet_xcm = 31,
		CumulusXcm: cumulus_pallet_xcm = 32,
		DmpQueue: cumulus_pallet_dmp_queue = 33,
		MessageQueue: pallet_message_queue = 34,

		// RWA + Crowdfunding (indices match Avatect mainnet deployment)
		Rwa: pallet_rwa = 40,
		Crowdfunding: pallet_crowdfunding = 41,

		Sudo: pallet_sudo = 255,
	}
);

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	frame_benchmarking::define_benchmarks!(
		[frame_system, SystemBench::<Runtime>]
		[pallet_utility, Utility]
		[pallet_balances, Balances]
		[pallet_assets, Assets]
		[pallet_multisig, Multisig]
		[pallet_identity, Identity]
		[pallet_trustless_agent, TrustlessAgent]
		[pallet_rwa, Rwa]
		[pallet_crowdfunding, Crowdfunding]
		[pallet_nfts, Nfts]
		[pallet_proxy, Proxy]
		[pallet_treasury, Treasury]
		[pallet_session, SessionBench::<Runtime>]
		[pallet_timestamp, Timestamp]
		[pallet_collator_selection, CollatorSelection]
		[cumulus_pallet_xcmp_queue, XcmpQueue]
	);
}

impl_runtime_apis! {
	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
		}

		fn authorities() -> Vec<AuraId> {
			pallet_aura::Authorities::<Runtime>::get().into_inner()
		}
	}

	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
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

		fn metadata_versions() -> sp_std::vec::Vec<u32> {
			Runtime::metadata_versions()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
		fn account_nonce(account: AccountId) -> Nonce {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
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
		fn query_call_info(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_call_info(call, len)
		}
		fn query_call_fee_details(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_call_fee_details(call, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl assets_api::AssetsApi<
		Block,
		AccountId,
		Balance,
		u32,
	> for Runtime
	{
		fn account_balances(account: AccountId) -> Vec<(u32, Balance)> {
			Assets::account_balances(account)
		}
	}

	impl pallet_nfts_runtime_api::NftsApi<Block, AccountId, u32, u32> for Runtime {
		fn owner(collection: u32, item: u32) -> Option<AccountId> {
			<Nfts as Inspect<AccountId>>::owner(&collection, &item)
		}

		fn collection_owner(collection: u32) -> Option<AccountId> {
			<Nfts as Inspect<AccountId>>::collection_owner(&collection)
		}

		fn attribute(
			collection: u32,
			item: u32,
			key: Vec<u8>,
		) -> Option<Vec<u8>> {
			<Nfts as Inspect<AccountId>>::attribute(&collection, &item, &key)
		}

		fn custom_attribute(
			account: AccountId,
			collection: u32,
			item: u32,
			key: Vec<u8>,
		) -> Option<Vec<u8>> {
			<Nfts as Inspect<AccountId>>::custom_attribute(
				&account,
				&collection,
				&item,
				&key,
			)
		}

		fn system_attribute(
			collection: u32,
			item: Option<u32>,
			key: Vec<u8>,
		) -> Option<Vec<u8>> {
			<Nfts as Inspect<AccountId>>::system_attribute(&collection, item.as_ref(), &key)
		}

		fn collection_attribute(collection: u32, key: Vec<u8>) -> Option<Vec<u8>> {
			<Nfts as Inspect<AccountId>>::collection_attribute(&collection, &key)
		}
	}

	impl pallet_rwa_runtime_api::RwaApi<Block, AccountId, Balance, BlockNumber, u32> for Runtime {
		fn effective_participation_status(
			asset_id: u32,
			participation_id: u32,
		) -> Option<pallet_rwa::ParticipationStatus<BlockNumber>> {
			Rwa::effective_participation_status(asset_id, participation_id)
		}

		fn can_participate(
			asset_id: u32,
			who: AccountId,
		) -> Result<(), pallet_rwa::CanParticipateError> {
			Rwa::can_participate(asset_id, who)
		}

		fn assets_by_owner(owner: AccountId) -> Vec<u32> {
			Rwa::assets_by_owner(owner)
		}

		fn participations_by_holder(holder: AccountId) -> Vec<(u32, u32)> {
			Rwa::participations_by_holder(holder)
		}

		fn active_participant_count(asset_id: u32) -> u32 {
			Rwa::active_participant_count(asset_id)
		}
	}

	impl pallet_crowdfunding_runtime_api::CrowdfundingApi<Block, AccountId, Balance, BlockNumber, u32> for Runtime {
		fn check_eligibility(
			campaign_id: u32,
			who: AccountId,
		) -> Result<(), pallet_crowdfunding::EligibilityError> {
			Crowdfunding::check_eligibility(campaign_id, who)
		}

		fn preview_withdrawal(
			campaign_id: u32,
			investor: AccountId,
			amount: Balance,
		) -> Option<pallet_crowdfunding::WithdrawalPreview<Balance>> {
			Crowdfunding::preview_withdrawal(campaign_id, investor, amount)
		}

		fn campaign_summary(campaign_id: u32) -> Option<pallet_crowdfunding::CampaignSummary<Balance, BlockNumber>> {
			Crowdfunding::campaign_summary(campaign_id)
		}

		fn campaigns_by_creator(creator: AccountId) -> Vec<u32> {
			Crowdfunding::campaigns_by_creator(creator)
		}

		fn campaigns_by_investor(investor: AccountId) -> Vec<u32> {
			Crowdfunding::campaigns_by_investor(investor)
		}

		fn get_investment(campaign_id: u32, investor: AccountId) -> Option<pallet_crowdfunding::Investment<Balance>> {
			Crowdfunding::get_investment(campaign_id, investor)
		}

		fn get_protocol_config() -> (u16, AccountId) {
			Crowdfunding::get_protocol_config()
		}
	}

	impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
		fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info(header)
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
			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, RuntimeBlockWeights::get().max_block)
		}

		fn execute_block(
			block: Block,
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
			use frame_system_benchmarking::Pallet as SystemBench;
			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();
			return (list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch};

			use frame_system_benchmarking::Pallet as SystemBench;
			impl frame_system_benchmarking::Config for Runtime {}

			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
			impl cumulus_pallet_session_benchmarking::Config for Runtime {}

			use frame_support::traits::WhitelistedStorageKeys;
			let whitelist = AllPalletsWithSystem::whitelisted_storage_keys();

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
}
