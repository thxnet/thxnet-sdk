<!-- Generated for thxnet-sdk v1.12.0 vs polkadot-sdk polkadot-v1.12.0. Re-run comparison when upgrading. -->

# Dimension 2: Runtimes

## Comparison Pairs

| THXNET. Runtime | Path | Upstream Equivalent | Notes |
|-----------------|------|-------------------|-------|
| thxnet-runtime | `thxnet/runtime/thxnet/src/lib.rs` | `polkadot/runtime/westend/src/lib.rs` | Mainnet rootchain |
| thxnet-testnet-runtime | `thxnet/runtime/thxnet-testnet/src/lib.rs` | `polkadot/runtime/westend/src/lib.rs` | Testnet rootchain |
| general-runtime | `thxnet/leafchain/runtime/general/src/lib.rs` | (none) | Pure THXNET. parachain |

Westend is the closest upstream equivalent because it's the actively-maintained testnet relay chain. Rococo may also be compared for parachain-specific features.

## Extracting construct_runtime! Pallets

THXNET. runtimes use the old-style `construct_runtime!` macro. Upstream may use either old-style or the newer `#[frame_support::runtime]` attribute macro.

```bash
THXNET_ROOT="$(pwd)"
UPSTREAM_ROOT="$(dirname "$THXNET_ROOT")/polkadot-sdk"

# Extract pallet lines from THXNET. runtime (old-style: "PalletName: module = INDEX,")
rg --no-filename '^\s+\w+:\s+\w+.*=\s*\d+' "$THXNET_ROOT/thxnet/runtime/thxnet/src/lib.rs" | \
  sed 's/^\s*//' | sort -t= -k2 -n

# Extract from upstream (may use #[runtime::pallet_index(N)])
rg --no-filename 'pallet_index\(\d+\)|:\s+\w+.*=\s*\d+' "$UPSTREAM_ROOT/polkadot/runtime/westend/src/lib.rs" | \
  sed 's/^\s*//' | sort -t= -k2 -n
```

## Pallet Index Map

Build a name → index map for both sides and classify:

| Index Range | Expected Pallets | THXNET. Specifics |
|-------------|-----------------|-------------------|
| 0-13 | System, Scheduler, Babe, Timestamp, Indices, Balances, Authorship, Staking, Offences, Session, Grandpa, AuthorityDiscovery | Core consensus — should match |
| 14-19 | Governance pallets | **DIVERGES**: THXNET. uses Gov V1, upstream may use OpenGov |
| 24-30 | Claims, Vesting, Utility, Identity, Proxy, Multisig | Utility — should match |
| 32-40 | TransactionPayment, Historical, Bounties, ElectionProvider, VoterList, ChildBounties, NominationPools, FastUnstake | Staking extensions — should match |
| 50-64 | Parachains pallets (Configuration, Shared, Inclusion, Inherent, Scheduler, Disputes, etc.) | Core parachain infra |
| 70-73 | Registrar, Slots, Auctions, Crowdloan | Parachain onboarding |
| 99 | XcmPallet | XCM |
| 131-135 | **NOT IN UPSTREAM** | AssetTxPayment (131), Assets (132), Nfts (133), Dao (134), FinalityRescue (135) |
| 250 | ParasSudoWrapper | Admin tooling — may not be in upstream production |
| 255 | Sudo | **THXNET. operational choice** — typically not in upstream production relays |

## Governance Model Difference

This is the biggest structural divergence:

| Aspect | THXNET. (Gov V1) | Upstream (OpenGov) |
|--------|-----------------|-------------------|
| Proposal mechanism | Democracy pallet | Referenda pallet |
| Deliberation | Council + TechnicalCommittee | Fellowship (Ranked Collective) |
| Voting | Simple majority / supermajority | Conviction voting with tracks |
| Privileged origins | Council/TechnicalCommittee ensure | Fellowship/track-specific origins |
| Election | PhragmenElection | (no council elections) |

**Impact**: Every `EnsureOrigin` configuration differs. All governance-related extrinsics have different semantics.

## Config Differences to Check

For each SHARED pallet, diff `impl pallet_X::Config for Runtime`:

```bash
# Example: compare Staking config
rg -A 50 'impl pallet_staking::Config for Runtime' "$THXNET_ROOT/thxnet/runtime/thxnet/src/lib.rs" | head -60
rg -A 50 'impl pallet_staking::Config for Runtime' "$UPSTREAM_ROOT/polkadot/runtime/westend/src/lib.rs" | head -60
```

Key parameters to compare:
- **Staking**: `MaxNominatorRewardedPerValidator`, `SessionsPerEra`, `BondingDuration`, slash ratios
- **Balances**: `ExistentialDeposit`
- **Identity**: deposit amounts, max fields, registrars
- **NominationPools**: `MaxPools`, `MaxMembersPerPool`
- **Parachains**: `Configuration` parameter defaults (max_code_size, hrmp limits, etc.)

## Leafchain Runtime

`thxnet/leafchain/runtime/general/` has no upstream equivalent. Document as pure ADDITION. It contains:
- Standard cumulus parachain pallets (ParachainSystem, ParachainInfo, Aura, etc.)
- THXNET.-specific pallets: Crowdfunding, RWA, TrustlessAgent
- Custom XCM configuration for THXNET. relay ↔ leafchain messaging
