<!-- Generated for thxnet-sdk v1.12.0 vs polkadot-sdk polkadot-v1.12.0. Re-run comparison when upgrading. -->

# Dimension 3: Custom Pallets

## Inventory

THXNET. has 5 wholly original pallets with zero upstream equivalents.

### Rootchain Pallets

#### pallet-dao (`thxnet/pallets/dao/`)

- **Purpose**: DAO governance mechanism for THXNET. rootchain
- **Source files**: `lib.rs`, `macros.rs`, `types.rs`, `mock.rs`, `tests.rs`
- **Runtime index**: 134 (thxnet-runtime), same index in thxnet-testnet-runtime
- **Capabilities**: No RPC, no runtime-api, no migrations, no attack tests
- **Config**: Custom DAO lifecycle (proposal, voting, execution)

#### pallet-finality-rescue (`thxnet/pallets/finality-rescue/`)

- **Purpose**: Emergency GRANDPA finality deadlock recovery
- **Source files**: `lib.rs`, `mock.rs`, `tests.rs`
- **Runtime index**: 135 (thxnet-runtime), same index in thxnet-testnet-runtime
- **Capabilities**: No RPC, no runtime-api, no migrations, no attack tests
- **Context**: Created in response to a real GRANDPA deadlock incident on THXNET. mainnet

### Leafchain Pallets

#### pallet-crowdfunding (`thxnet/leafchain/pallets/crowdfunding/`)

- **Purpose**: Campaign lifecycle management for crowdfunding on leafchain
- **Source files**: `lib.rs` (~63KB), `types.rs`, `migrations.rs`, `mock.rs`, `tests.rs`, `benchmarks.rs`, `attack_tests.rs`
- **RPC**: `crowdfunding/rpc/` — query campaign state
- **Runtime API**: `crowdfunding/runtime-api/` — runtime-level campaign queries
- **Migrations**: `migrations.rs` (~4.3KB) — storage version upgrades
- **Attack Tests**: `attack_tests.rs` — adversarial scenario coverage
- **Benchmarks**: Yes

#### pallet-rwa (`thxnet/leafchain/pallets/rwa/`)

- **Purpose**: Real-world asset tokenization and lifecycle management
- **Source files**: `lib.rs` (~76KB), `types.rs`, `migrations.rs`, `mock.rs`, `tests.rs`, `benchmarking.rs`, `attack_tests.rs` (~83KB)
- **RPC**: `rwa/rpc/` — query RWA state
- **Runtime API**: `rwa/runtime-api/` — runtime-level RWA queries
- **Migrations**: `migrations.rs` (~3.6KB) — storage version upgrades
- **Attack Tests**: `attack_tests.rs` (~83KB, most extensive attack test suite)
- **Benchmarks**: Yes

#### pallet-trustless-agent (`thxnet/leafchain/pallets/trustless-agent/`)

- **Purpose**: Trustless agent operations on leafchain
- **Source files**: `lib.rs` (~77KB), `migrations.rs`, `mock.rs`, `tests.rs`, `test_constants.rs`, `benchmarking.rs`
- **RPC**: None
- **Runtime API**: None
- **Migrations**: `migrations.rs` (~4.7KB) — storage version upgrades
- **Attack Tests**: None
- **Benchmarks**: Yes

## Capabilities Matrix

| Capability | dao | finality-rescue | crowdfunding | rwa | trustless-agent |
|-----------|-----|----------------|-------------|-----|----------------|
| RPC endpoint | - | - | Yes | Yes | - |
| Runtime API | - | - | Yes | Yes | - |
| Storage migrations | - | - | Yes | Yes | Yes |
| Attack tests | - | - | Yes | Yes | - |
| Benchmarks | - | - | Yes | Yes | Yes |
| In rootchain runtime | Yes | Yes | - | - | - |
| In leafchain runtime | - | - | Yes | Yes | Yes |

## Verification Checklist

For each custom pallet, verify:

1. **Runtime wiring**: Is it in the correct `construct_runtime!` with the expected index?
2. **Weight implementation**: Does it have benchmarks? Are weights up to date?
3. **Test coverage**: Does it have unit tests? Attack tests?
4. **Migration coverage**: If it has storage versioning, are all version transitions covered?
5. **RPC/Runtime API wiring**: If it has RPC endpoints, are they properly registered in the node service?
6. **Feature flags**: Does it properly gate `runtime-benchmarks` and `try-runtime` features?

```bash
# Quick check: verify pallet is in construct_runtime!
rg 'Dao|FinalityRescue|Crowdfunding|Rwa|TrustlessAgent' thxnet/runtime/*/src/lib.rs thxnet/leafchain/runtime/*/src/lib.rs

# Quick check: verify benchmarks exist
ls thxnet/pallets/*/src/benchmarking.rs thxnet/leafchain/pallets/*/src/benchmarking.rs 2>/dev/null

# Quick check: verify migration modules
ls thxnet/leafchain/pallets/*/src/migrations.rs 2>/dev/null
```
