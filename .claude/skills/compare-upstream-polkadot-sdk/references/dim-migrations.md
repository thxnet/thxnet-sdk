<!-- Generated for thxnet-sdk v1.12.0 vs polkadot-sdk polkadot-v1.12.0. Re-run comparison when upgrading. -->

# Dimension 6: Migration Chain Analysis

## Why This Dimension is Critical

The **Chain Integrity Invariant** requires:
1. **Genesis-to-tip sync** — A freshly started node must sync from genesis to latest block
2. **In-place upgrade** — An existing node must upgrade to the latest version without manual intervention

The migration chain is the mechanism that ensures storage format compatibility across versions. Missing or misordered migrations break chain sync.

## THXNET. Migration Chain Structure

The thxnet runtime defines cumulative migrations at the end of `lib.rs`:

```rust
pub type Migrations = migrations::Unreleased;

pub mod migrations {
    pub type Unreleased = (MigrationsEarly, MigrationsLate);
    // MigrationsEarly = v0.9.40 through ~v1.5.0
    // MigrationsLate  = ~v1.5.0 through v1.12.0
}
```

**All migrations are cumulative** — they are never removed. This is required because a fresh node syncing from genesis must replay all migrations in order.

## Extracting Migration Entries

```bash
THXNET_ROOT="$(pwd)"
UPSTREAM_ROOT="$(dirname "$THXNET_ROOT")/polkadot-sdk"

# Extract the Unreleased migration type from THXNET. runtime
rg -A 100 'pub mod migrations' "$THXNET_ROOT/thxnet/runtime/thxnet/src/lib.rs" | head -120

# Extract equivalent from upstream
rg -A 100 'type Migrations\b|pub mod migrations' "$UPSTREAM_ROOT/polkadot/runtime/westend/src/lib.rs" | head -120
```

## THXNET.-Specific Migration Entries

These entries exist in thxnet-sdk but NOT in upstream at the same version:

### 1. parachains_configuration v5 and v6

```rust
parachains_configuration::migration::v5::MigrateToV5<Runtime>
parachains_configuration::migration::v6::MigrateToV6<Runtime>
```

**Why**: Upstream removed v5/v6 migration code after v1.0.0 (Polkadot/Kusama had already executed them). THXNET. rootchain was still at configuration v4 when upgrading from v0.9.40, so these must be present.

**Status**: Already executed on all rootchain nodes. No-op on re-run (guarded by on-chain version).

### 2. StakingBridgeV13ToV14

```rust
type StakingBridgeV13ToV14 = VersionedMigration<13, 14, pallet_staking::migrations::v14::MigrateToV14<Runtime>, pallet_staking::Pallet<Runtime>, RocksDbWeight>;
```

**Why**: Upstream `MigrateToV14` has a guard `in_code_version == 14` which is dead in v1.12.0 (in_code is now 15+). This custom wrapper uses `VersionedMigration` which correctly checks `on_chain_version == 13 → 14`.

### 3. FixGrandpaFinalityDeadlock

```rust
FixGrandpaFinalityDeadlock
```

**Why**: THXNET.-specific fix for a GRANDPA finality deadlock (originally at spec_version 94000004). Already executed on mainnet. Contains a block number guard — no-op when block > 14,250,000.

### 4. StampBountiesV4

```rust
StampBountiesV4
```

**Why**: `pallet_bounties` declares in-code version 4 but on-chain is 0 (NULL). The upstream v4 migration was a prefix rename from `Treasury` to `Bounties` — irrelevant since THXNET. always used `Bounties` prefix. This stamps the version to 4 without data transformation.

### 5. StampParasDisputesV1

```rust
StampParasDisputesV1
```

**Why**: `parachains_disputes` declares in-code version 1 but on-chain is 0. The upstream v0→v1 migration never ran on THXNET. (was already removed by the version THXNET. started from). This stamps the version to 1.

### 6. UpgradeSessionKeys

```rust
UpgradeSessionKeys
```

**Why**: Removes the ImOnline key from session keys. ImOnline pallet was removed in v1.5.0 but session key storage still contained the old key format.

### 7. NominationPools v4-to-v5 wrapper

```rust
pallet_nomination_pools::migration::versioned::V4toV5<Runtime>
```

**Why**: The original `MigrateToV5` has a broken guard (`in_code == 5`) that never fires in v1.12.0 (in_code is now 8+). Uses `VersionedMigration` wrapper for correct on-chain version check.

## Classification Method

For each migration entry in the chain:

| Classification | Meaning |
|---------------|---------|
| SHARED | Same entry exists in both repos (standard upstream migration) |
| THXNET-ONLY | Entry unique to THXNET. (ported code, custom wrapper, version stamp, or THXNET.-specific fix) |
| UPSTREAM-ONLY | Entry exists in upstream but not in THXNET. (may indicate a gap — investigate!) |

**UPSTREAM-ONLY is a red flag** — it may mean THXNET. is missing a migration that upstream added. Always investigate.

## Ordering Invariant

Migration execution order matters. Migrations run in tuple order (left to right, nested tuples flattened). A migration that depends on storage state set by a prior migration must come after it.

THXNET.-specific migrations must be positioned correctly relative to upstream migrations:
- Version stamps should come AFTER the upstream migration that introduced the version check
- Custom wrappers should replace (not duplicate) the upstream migration they fix

## Leafchain Per-Pallet Migrations

Leafchain pallets have their own migration modules:

| Pallet | Migration File | Purpose |
|--------|---------------|---------|
| pallet-crowdfunding | `thxnet/leafchain/pallets/crowdfunding/src/migrations.rs` | Storage version upgrades |
| pallet-rwa | `thxnet/leafchain/pallets/rwa/src/migrations.rs` | Storage version upgrades |
| pallet-trustless-agent | `thxnet/leafchain/pallets/trustless-agent/src/migrations.rs` | Storage version upgrades |

These have zero upstream equivalents — they are pure THXNET. additions.

```bash
# Check migration modules exist and their content
wc -l thxnet/leafchain/pallets/*/src/migrations.rs 2>/dev/null
```
