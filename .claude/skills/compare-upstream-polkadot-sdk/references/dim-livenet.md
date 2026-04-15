<!-- Generated for thxnet-sdk v1.12.0 vs polkadot-sdk polkadot-v1.12.0. Re-run comparison when upgrading. -->

# Dimension 7: Livenet Deployment & Operations

## Chain Topology

THXNET. operates 11 live blockchain networks across 3 runtimes:

| Runtime | Chains | Role |
|---------|--------|------|
| `thxnet-runtime` | rootchain mainnet | Relay chain (mainnet) |
| `thxnet-testnet-runtime` | rootchain testnet | Relay chain (testnet) |
| `general-runtime` | ALL leafchains | Shared parachain runtime for sand, avatect, lmt, thx, etc. |

### Chain List

| Chain | Network | Runtime | CI-Tested | RPC Secret |
|-------|---------|---------|-----------|------------|
| rootchain mainnet | mainnet | thxnet-runtime | Yes | `ROOTCHAIN_MAINNET_RPC_URL` |
| rootchain testnet | testnet | thxnet-testnet-runtime | Yes | `ROOTCHAIN_RPC_URL` |
| sand-testnet | testnet | general-runtime | Yes | `LEAFCHAIN_RPC_URL` |
| avatect-mainnet | mainnet | general-runtime | Yes | `LEAFCHAIN_MAINNET_RPC_URL` |
| lmt-testnet | testnet | general-runtime | Yes | `LEAFCHAIN_LMT_TESTNET_RPC_URL` |
| lmt-mainnet | mainnet | general-runtime | Yes | `LEAFCHAIN_LMT_MAINNET_RPC_URL` |
| thx | — | general-runtime | No | — |
| (others) | — | general-runtime | No | — |

**Key insight**: All leafchains share the same `general-runtime` WASM. A runtime upgrade to `general-runtime` affects ALL leafchains simultaneously.

## Runtime Upgrade Lifecycle

A runtime upgrade follows this sequence:

```
1. Code change (new pallet, migration, config change)
      ↓
2. spec_version bump in runtime lib.rs
      ↓
3. Build WASM (cargo build --release or release.yml)
      ↓
4. try-runtime test against live chain state (CI or manual)
      ↓
5. WASM upload to chain via Sudo::set_code or Democracy
      ↓
6. Runtime enactment (next block after authorized)
      ↓
7. Migrations execute (on_runtime_upgrade)
      ↓
8. Verify: block production, finality, feature correctness
```

### Rollout Order

**Always testnet first, then mainnet**:

```
Rootchain: testnet rootchain → validate → mainnet rootchain
Leafchain: sand-testnet → validate → lmt-testnet → validate → avatect-mainnet → lmt-mainnet
```

Never skip testnet validation. If a migration or feature fails on testnet, fix before proceeding to mainnet.

## Feature Toggles

Features can be enabled/disabled via Sudo or governance calls. Common patterns:

### Pallet-Level Toggles

```
# Enable a new pallet (already in construct_runtime! but needs activation)
Sudo::sudo(set_storage(...))  # Set pallet storage version or activation flag

# Disable a pallet (emergency)
Sudo::sudo(set_storage(...))  # Clear activation flag or pause mechanism
```

### Parameter Changes

```
# Change staking parameters
Sudo::sudo(Staking::force_new_era_always())
Sudo::sudo(Staking::set_validator_count(N))

# Change parachain configuration
Sudo::sudo(Configuration::set_max_code_size(N))
Sudo::sudo(Configuration::set_hrmp_max_message_num_per_candidate(N))

# Toggle pallet features
Sudo::sudo(call_to_pallet_specific_toggle)
```

### Per-Chain Feature Matrix

Track which features are enabled on which chain:

```markdown
| Feature | testnet rootchain | mainnet rootchain | sand-testnet | avatect-mainnet | lmt-testnet | lmt-mainnet |
|---------|-------------------|-------------------|-------------|----------------|-------------|-------------|
| Gov V1 | Yes | Yes | N/A | N/A | N/A | N/A |
| Sudo | Yes | Yes | Yes | Yes | Yes | Yes |
| Crowdfunding | N/A | N/A | Yes | Yes | Yes | Yes |
| RWA | N/A | N/A | Yes | Yes | Yes | Yes |
| TrustlessAgent | N/A | N/A | Yes | Yes | Yes | Yes |
| ... | ... | ... | ... | ... | ... | ... |
```

## Chain-Data Operations

One-off operations that must be applied to specific chains. These are NOT repeatable migrations — they are operational actions.

### Categories

| Category | Example | How Applied |
|----------|---------|-------------|
| Version stamps | StampBountiesV4, StampParasDisputesV1 | Via migration in runtime upgrade |
| Storage fixes | FixGrandpaFinalityDeadlock | Via migration with block-number guard |
| Key rotations | UpgradeSessionKeys | Via migration (removes ImOnline key) |
| Data migrations | NominationPools wrapper | Via migration with VersionedMigration |
| Parameter updates | Staking/Configuration changes | Via Sudo call after upgrade |
| Emergency actions | Finality rescue, validator rotation | Via Sudo or pallet-specific extrinsic |

### Deployment Matrix Template

Track which operations have been applied to which chains:

```markdown
| Operation | Status | testnet rootchain | mainnet rootchain | Leafchains |
|-----------|--------|-------------------|-------------------|------------|
| v1.12.0 runtime upgrade | Done/Pending | Applied (block #X) | Applied (block #Y) | Applied |
| StampBountiesV4 | Done | Yes (via migration) | Yes (via migration) | N/A |
| FixGrandpaFinalityDeadlock | Done | Yes (no-op, block > guard) | Yes (executed at block ~14M) | N/A |
| UpgradeSessionKeys | Done | Yes | Yes | N/A |
| New staking parameters | Pending | Sudo call done | Sudo call pending | N/A |
| ... | ... | ... | ... | ... |
```

## Verification Methods

After any deployment operation, verify:

### Block Production

```bash
# Check chain is producing blocks
# Via polkadot.js or substrate-api-sidecar
curl -s -H "Content-Type: application/json" \
  -d '{"id":1, "jsonrpc":"2.0", "method":"chain_getHeader"}' \
  "$RPC_URL"
```

### Runtime Version

```bash
# Verify spec_version matches expected
curl -s -H "Content-Type: application/json" \
  -d '{"id":1, "jsonrpc":"2.0", "method":"state_getRuntimeVersion"}' \
  "$RPC_URL" | python3 -m json.tool
```

### Storage Version Check

```bash
# Verify pallet storage versions are correct after migration
# Use polkadot.js Apps → Developer → Chain state → System.palletVersion
```

### try-runtime (Pre-deployment)

```bash
# Test migrations against live state before deploying
cargo build --release --features try-runtime
./target/release/polkadot try-runtime \
  --runtime ./target/release/wbuild/thxnet-runtime/thxnet_runtime.compact.compressed.wasm \
  on-live --uri "$RPC_URL"
```

## Relationship to Other Dimensions

Dim 7 depends on all other dimensions:
- **Dim 2 (Runtimes)**: What pallets are in each runtime determines what features exist per chain
- **Dim 3 (Pallets)**: Custom pallet features need enabling on specific chains
- **Dim 6 (Migrations)**: Every migration entry must eventually be deployed to all relevant chains
- **Dim 4 (CI)**: try-runtime CI jobs validate against live chain state

When doing a full comparison, process Dims 1-6 first to understand the code delta, then use Dim 7 to reason about deployment impact.
