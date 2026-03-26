# THXNet SDK v1.12.0 Endgame — TODO Tracker

> Branch: `feat/endgame-v1.12.0`
> Target: polkadot-sdk v1.12.0 (commit b4016902ac)
> Last updated: 2026-03-26

---

## Phase 1: Migrations & Compilation — DONE ✅

### Rootchain Runtime (mainnet + testnet)

- [x] Branch setup: v1.12.0 base + thxnet custom code merge
- [x] Zero-fee config (TRANSACTION_BYTE_FEE=0, OPERATIONAL_FEE_MULTIPLIER=0, LengthToFee=WeightToFee)
- [x] Configuration v4→v5 migration (ported from polkadot v1.0.0)
- [x] Configuration v5→v6 migration (ported from polkadot v1.0.0)
- [x] Configuration v6→v7→v8→v9→v10→v11→v12 (already existed)
- [x] NominationPools v4→v5 (custom VersionedMigration wrapper, broken guard fixed)
- [x] NominationPools v5→v6→v7→v8 (already existed)
- [x] Staking v13→v14 bridge (custom VersionedMigration, upstream guard dead in v1.12.0)
- [x] Staking v14→v15 (v1.12.0 new)
- [x] GRANDPA v4→v5 (already existed)
- [x] FixGrandpaFinalityDeadlock runtime migration (ported from rootchain, noop at current block)
- [x] FinalityRescue pallet added at index 135
- [x] Offences v0→v1
- [x] Tips RemovePallet
- [x] ImOnline RemovePallet + UpgradeSessionKeys
- [x] Scheduler (parachains) v0→v1→v2
- [x] Crowdloan v1→v2
- [x] Identity v0→v1
- [x] XCM MigrateToLatest
- [x] Registrar v0→v1
- [x] ParaInclusion v0→v1
- [x] Bounties v0→v4 stamp (no data migration, prefix rename is noop)
- [x] spec_version: 112_000_001

### Leafchain Runtime (unified general-runtime for all 9 chains)

- [x] CollatorSelection v0→v1→v2
- [x] XcmpQueue v2→v3→v4 (handles ECQ at v2 AND others at v3)
- [x] DmpQueue force stamp v0→v2 (zero data on all chains)
- [x] Rwa v0→v5 stamp
- [x] Crowdfunding stamp v0→v3 (custom: replaces broken MigrateToV3 guard)
- [x] TrustlessAgent v0→v1
- [x] Treasury pallet restored at index 19
- [x] burn_from API adapted (Preservation::Expendable)
- [x] try-runtime error types fixed (pallet-rwa + pallet-crowdfunding)
- [x] spec_version: 16

### Compilation

- [x] `thxnet-runtime` compiles (dev + try-runtime)
- [x] `thxnet-testnet-runtime` compiles (dev + try-runtime)
- [x] `general-runtime` compiles (dev + try-runtime)
- [x] Release build: `polkadot` binary (121 MB)
- [x] Release build: `thxnet-leafchain` binary (153 MB)
- [x] Release build: `thxnet_runtime.compact.compressed.wasm` (2.0 MB)
- [x] Release build: `general_runtime.compact.compressed.wasm` (1.3 MB)

### Testing

- [x] 35 forensic migration unit tests (MECE coverage)
- [x] 8 Bounties stamp behavioral tests
- [x] Configuration v5 migration unit tests (2 tests)
- [x] All rootchain tests pass (27 mainnet + 24 testnet)
- [x] All leafchain tests pass (19)

---

## Phase 2: try-runtime Validation — TODO

### How to Run

```bash
# Pre-flight check
./scripts/try-runtime-test.sh check

# Test against testnet (DOES NOT AFFECT LIVE CHAINS — read-only state fetch)
./scripts/try-runtime-test.sh test-all-testnet

# Test against mainnet (only after testnet passes)
./scripts/try-runtime-test.sh test-rootchain-mainnet
./scripts/try-runtime-test.sh test-leafchain-avatect
```

### What try-runtime Does

1. Connects to live archive nodes via WebSocket (READ-ONLY)
2. Downloads full chain state snapshot to local memory
3. Applies all runtime migrations locally
4. Runs pre_upgrade() and post_upgrade() checks
5. Validates all StorageVersion values match (on-chain == in-code)
6. Reports any errors or panics

**⚠️ Resource requirements**: ~16-32 GB RAM for rootchain state, ~4-8 GB for leafchain.
**⚠️ Time**: 15-60 minutes per chain depending on state size.
**⚠️ Safety**: 100% read-only. No transactions sent. No state modified on live chains.

### Chains to Validate

- [ ] Testnet rootchain: `wss://node.testnet.thxnet.org/archive-001/ws`
- [ ] Testnet Sand (leafchain): `wss://node.sand.testnet.thxnet.org/archive-001/ws`
- [ ] Mainnet rootchain: `wss://node.mainnet.thxnet.org/archive-001/ws`
- [ ] Mainnet Avatect (leafchain): `wss://node.avatect.mainnet.thxnet.org/archive-001/ws`
- [ ] Mainnet THX (leafchain): `wss://node.thx.mainnet.thxnet.org/archive-001/ws`
- [ ] Mainnet LMT (leafchain): `wss://node.lmt.mainnet.thxnet.org/archive-001/ws`

### Expected Outcomes

For EACH chain:
- [ ] All migrations execute without panic
- [ ] All StorageVersion values match (on-chain == in-code)
- [ ] No "migration skipped" warnings for critical items
- [ ] Weight consumption within block limits
- [ ] pre_upgrade/post_upgrade checks all pass

---

## Phase 3: Forked Testnet Deployment — TODO

### Strategy

Deploy to k8s-isolated forked testnet (live testnet replica, network isolated):

1. [ ] k8s environment setup (isolated network)
2. [ ] Deploy new rootchain binary (10 validators, rolling restart)
3. [ ] Verify rootchain finality and block production
4. [ ] `sudo.setCode()` rootchain runtime WASM
5. [ ] Verify all rootchain migrations executed correctly
6. [ ] Deploy new leafchain binary (all collators)
7. [ ] `sudo.sudoUncheckedWeight(system.setCode())` per leafchain
8. [ ] Verify all leafchain migrations executed correctly

### Post-Deployment Verification

- [ ] NFTs intact: 90,353 (THX) + 79,483 (LMT) + others
- [ ] Balances intact: all accounts
- [ ] Staking: 10 validators active, producing blocks
- [ ] GRANDPA finality: blocks finalize < 12s
- [ ] Crowdfunding: 21 campaigns on Avatect accessible
- [ ] RWA: 38 assets on Avatect accessible
- [ ] TrustlessAgent: 12 agents on Sand operational
- [ ] Dao: 40 entries (mainnet) / 8538 entries (testnet)
- [ ] Zero-fee: tx cost = 0
- [ ] XCM: cross-chain messages work
- [ ] Parachain validation: rootchain validates leafchain blocks

---

## Phase 4: Live Deployment — TODO

### Testnet First
- [ ] Deploy rootchain binary to live testnet
- [ ] Upgrade rootchain runtime on live testnet
- [ ] Deploy leafchain binary to live testnet
- [ ] Upgrade all 5 testnet leafchain runtimes
- [ ] Monitor 24h

### Mainnet
- [ ] Deploy rootchain binary to live mainnet
- [ ] Upgrade rootchain runtime on live mainnet
- [ ] Deploy leafchain binary to live mainnet
- [ ] Upgrade all 4 mainnet leafchain runtimes
- [ ] Monitor 48h
- [ ] Tag: `thxnet-sdk-v1.12.0`

---

## Phase 5: Remaining Items — TODO

### Should Do
- [ ] zombienet test (XCM, parachain validation, block production)
- [ ] Fix test compilation issues (SignedDepositBase, EncodeLike in test code)
- [ ] Cargo clippy cleanup
- [ ] Benchmark weights re-run

### Nice To Have
- [ ] DevOps deployment guide document
- [ ] Chain spec regeneration (if needed for new deployments)
- [ ] CI: add rootchain + leafchain full test suite

---

## Live Chain Reference

| Chain | Endpoint | spec_version | Key Data |
|---|---|---|---|
| Mainnet Relay | wss://node.mainnet.thxnet.org/archive-001/ws | 94000004 | 10 validators, ~14.68M blocks |
| Testnet Relay | wss://node.testnet.thxnet.org/archive-001/ws | 94000004 | ~15.38M blocks |
| Mainnet Avatect | wss://node.avatect.mainnet.thxnet.org/archive-001/ws | 3 | Rwa=38, CF=21 |
| Testnet Sand | wss://node.sand.testnet.thxnet.org/archive-001/ws | 3 | TA=12, Escrows=6 |
| Mainnet THX | wss://node.thx.mainnet.thxnet.org/archive-001/ws | 2 | 5,315 accts, 90,353 NFTs |
| Mainnet LMT | wss://node.lmt.mainnet.thxnet.org/archive-001/ws | 2 | 13,226 accts, 79,483 NFTs |
| Mainnet ECQ | wss://node.ecq.mainnet.thxnet.org/archive-001/ws | 2 | 3 accounts |
| Testnet THX | wss://node.thx.testnet.thxnet.org/archive-001/ws | 2 | 2,448 accts, 3,108 NFTs |
| Testnet LMT | wss://node.lmt.testnet.thxnet.org/archive-001/ws | 2 | 5,982 accts, 66,900 NFTs |
| Testnet Izutsuya | wss://node.izutsuya.testnet.thxnet.org/archive-001/ws | 2 | 16 accts, 1,549 NFTs |
| Testnet ECQ | wss://node.ecq.testnet.thxnet.org/archive-001/ws | 2 | 3 accounts |
