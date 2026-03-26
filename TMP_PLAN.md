# Plan: Staking Bridge Migration + Node Binary / Runtime Upgrade Safety

## Context

We're upgrading THX rootchain (relay) + 4 leafchains (parachains) from Substrate v0.9.40 to polkadot-sdk v1.10.0. This is a **massive version jump** across a monorepo merge. Live mainnet has real validators, collators, and user data.

### The Staking Bug

Live rootchain pallet-staking (v0.9.40) stores its version as `ObsoleteReleases::V12_0_0` in a custom `StorageValue`. The v1.10.0 code declares `STORAGE_VERSION = 14`. The upstream v13 bridge migration guard (`in_code == 13`) can never match because our in-code is 14. The v14 migration guard (`on_chain == 13`) also fails because framework `StorageVersion` was never set (reads 0). Both silently no-op.

### The Broader Survival Question

- **Node binary**: v0.9.40 `NativeElseWasmExecutor` → v1.10.0 `WasmExecutor` (native removed). Can existing RocksDB/ParityDB survive?
- **Runtime WASM**: Massive API changes. 19 rootchain migrations, 8 leafchain migrations. Will they all fire correctly?
- **Validators**: 10 validators need rolling restart. Can they survive partial upgrade?
- **Parachains**: 4 leafchains (THX/LMT/AVATECT/ECQ). Collators need restart too.

---

## Part 1: Staking Bridge Migration

### Approach

Create runtime-level custom migration in `thxnet/runtime/thxnet/src/migrations.rs`. Do NOT modify upstream pallet-staking.

### Files to Create/Modify

| File                                              | Action                                                                             |
| ------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `thxnet/runtime/thxnet/src/migrations.rs`         | NEW — `StakingV12ObsoleteToV14` migration + unit tests                             |
| `thxnet/runtime/thxnet/src/lib.rs`                | Add `mod migrations;`, insert migration BEFORE `MigrateToV14` in `MigrationsEarly` |
| `thxnet/runtime/thxnet-testnet/src/migrations.rs` | NEW — same migration                                                               |
| `thxnet/runtime/thxnet-testnet/src/lib.rs`        | Same wiring                                                                        |

### Migration Code Structure

```rust
// thxnet/runtime/thxnet/src/migrations.rs

use codec::{Decode, Encode};
use frame_support::{storage_alias, traits::OnRuntimeUpgrade, weights::Weight};
use pallet_staking::{Config, Pallet as StakingPallet};

/// Legacy version enum from Substrate v0.9.40 pallet-staking.
/// Kept here for SCALE decode compatibility with on-chain storage.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, Debug)]
enum ObsoleteReleases {
    V1_0_0Ancient, V2_0_0, V3_0_0, V4_0_0, V5_0_0, V6_0_0,
    V7_0_0, V8_0_0, V9_0_0, V10_0_0, V11_0_0, V12_0_0,
}

impl Default for ObsoleteReleases {
    fn default() -> Self { ObsoleteReleases::V12_0_0 }
}

/// Storage alias to read/kill the legacy version key
/// (same prefix as pallet-staking's internal storage_alias)
#[storage_alias]
type StorageVersion<T: Config> = StorageValue<StakingPallet<T>, ObsoleteReleases, ValueQuery>;

pub struct StakingV12ObsoleteToV14<T>(core::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for StakingV12ObsoleteToV14<T> {
    fn on_runtime_upgrade() -> Weight {
        let framework_version = StakingPallet::<T>::on_chain_storage_version();

        // If framework version already >= 14, skip (already migrated or fresh chain)
        if framework_version >= 14 {
            log::info!("staking-bridge: framework version {} >= 14, skipping", framework_version);
            return T::DbWeight::get().reads(1);
        }

        // Check if legacy key exists with V12_0_0
        let legacy = StorageVersion::<T>::get();
        if legacy != ObsoleteReleases::V12_0_0 {
            log::warn!("staking-bridge: legacy version {:?} != V12_0_0, skipping", legacy);
            return T::DbWeight::get().reads(2);
        }

        // Bridge: kill legacy key + stamp framework v14
        StorageVersion::<T>::kill();
        frame_support::traits::StorageVersion::new(14).put::<StakingPallet<T>>();

        log::info!("staking-bridge: migrated from ObsoleteReleases::V12_0_0 to StorageVersion(14)");
        T::DbWeight::get().reads_writes(2, 2)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
        let framework = StakingPallet::<T>::on_chain_storage_version();
        let legacy_exists = StorageVersion::<T>::exists();
        log::info!("staking-bridge pre_upgrade: framework={}, legacy_exists={}", framework, legacy_exists);
        Ok((framework.encode(), legacy_exists).encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
        frame_support::ensure!(
            StakingPallet::<T>::on_chain_storage_version() >= 14,
            "staking-bridge: framework version should be >= 14"
        );
        frame_support::ensure!(
            !StorageVersion::<T>::exists(),
            "staking-bridge: legacy StorageVersion key should be killed"
        );
        Ok(())
    }
}
```

### Migrations Tuple Wiring

```rust
// In MigrationsEarly, insert BEFORE the existing v14:
type MigrationsEarly = (
    // ... parachains_configuration v7/v8/v9, paras_registrar v1, nomination_pools v5/v6/v7 ...
    // v1.3.0 → v1.4.0
    migrations::StakingV12ObsoleteToV14<Runtime>,    // NEW: bridge legacy → framework v14
    pallet_staking::migrations::v14::MigrateToV14<Runtime>,  // upstream: noop after bridge
    pallet_grandpa::migrations::MigrateV4ToV5<Runtime>,
    // ... rest unchanged ...
);
```

### Behavioural Tests (7 tests)

All tests use `sp_io::TestExternalities` with a minimal mock runtime that includes `pallet_staking`. Manually insert legacy `ObsoleteReleases::V12_0_0` into raw storage to simulate live chain.

1. **`bridge_happy_path`**: Insert `V12_0_0` → run migration → verify `StorageVersion == 14` AND legacy key killed
2. **`already_at_v14`**: Set framework `StorageVersion(14)` → run → verify noop (0 writes)
3. **`legacy_key_missing_framework_at_zero`**: No legacy key, framework = 0 → run → verify noop (doesn't crash, doesn't stamp)
4. **`idempotent_double_run`**: Run twice → second is noop, result identical
5. **`upstream_v14_noop_after_bridge`**: Run bridge → then run upstream `MigrateToV14` → verify it's a safe noop
6. **`try_runtime_pre_upgrade_captures_state`**: Verify `pre_upgrade()` runs without error
7. **`try_runtime_post_upgrade_validates`**: Run migration → verify `post_upgrade()` succeeds

---

## Part 2: Node Binary Replacement Safety

### Verified Facts

| Question                              | Answer   | Evidence                                                                                   |
| ------------------------------------- | -------- | ------------------------------------------------------------------------------------------ |
| Can new binary open old RocksDB?      | **YES**  | Same `sc_client_db` layer, no DB schema break                                              |
| Can new binary open old ParityDB?     | **YES**  | `parachains_db/upgrade.rs` has v0→v1→v2→v3 migration paths for ParityDB                    |
| Can new binary execute old WASM?      | **YES**  | All v0.9.40 host functions kept via `#[version(N, register_only)]`                         |
| Can old binary execute new WASM?      | **NO**   | New WASM may call host functions that don't exist in old binary → **TRAP**                 |
| NativeElseWasm → WasmExecutor change? | **SAFE** | v1.10.0 still has NativeElseWasm but native is deprecated; WASM execution is deterministic |

### Critical Ordering Rule

**Binary FIRST, then Runtime WASM.** Never the other way around. Old binary + new WASM = potential host function trap = chain halt.

### Rootchain Validator Rolling Restart

- 10 validators, GRANDPA needs ⅔+1 (7) for finality
- Safe to have up to 3 offline simultaneously
- Restart 1 at a time, wait for sync + finality, then next
- ALL 10 must be on new binary BEFORE `sudo.setCode()` runtime upgrade

### Leafchain Collator Restart

- Collators produce blocks, relay chain validates them
- Old collator binary can produce blocks for old runtime (fine during transition)
- ALL collators must be on new binary before parachain runtime upgrade
- PVF pre-checking adds ~60 min delay after `authorize_upgrade`

---

## Part 3: Additional Safety Items to Wire

### Rootchain: FinalityRescue pallet (index 135)

Old rootchain (spec v94000005 in codebase) has `FinalityRescue = 135`. New thxnet-sdk does NOT include it in `construct_runtime!`. This is safe because:

- v94000005 was never enacted on-chain (live is v94000004)
- No storage was ever written under this prefix
- No `RemovePallet` needed

### Rootchain: `NativeElseWasmExecutor` Compatibility

Old rootchain uses `NativeElseWasmExecutor`. New thxnet-sdk v1.10.0:

- Still supports this executor type
- WASM execution is the primary path
- Native runtime is deprecated but functional
- The `can_author_with` check is relaxed

### Leafchain: New Pallets (Identity=28, TrustlessAgent=27, MessageQueue=34)

These didn't exist in old leafchains. They initialize with empty storage on first access. No migration needed. Pallet indices don't conflict with anything existing.

---

## Verification Plan

1. `cargo test -p thxnet-runtime --lib migrations` — 7 staking bridge tests
2. `cargo test -p thxnet-testnet-runtime --lib migrations` — same for testnet
3. `SKIP_WASM_BUILD=true cargo check -p thxnet-runtime -p thxnet-testnet-runtime` — compile check
4. `cargo test -p pallet-rwa -p pallet-crowdfunding -p pallet-dao -p pallet-finality-rescue` — regression (1412 tests)
5. `cargo test -p polkadot-service --lib thxnet` — GRANDPA tests (9 tests)
6. Future: `try-runtime on-runtime-upgrade --live --uri wss://node.mainnet.thxnet.org/archive-001/ws` against live state
