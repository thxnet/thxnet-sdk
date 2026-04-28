---
name: testing-migration-idempotency
description: >
  Verifies runtime migration idempotency using Chopsticks to fork live state,
  bypassing the try-runtime v0.10.1 --create-snapshot limitation. Forks the
  same block twice, runs migrations on each fork independently, and compares
  state roots and pallet storage to prove determinism. Covers all 6 CI chains.
  Use when: idempotency test, migration determinism, snapshot workaround,
  try-runtime snapshot blocker, migration reproducibility, run migration twice,
  chopsticks idempotency, migration safety, 冪等性測試, 遷移確定性.
---

# Migration Idempotency Testing (Chopsticks-based)

## Problem

The existing `try-runtime-test.sh test-idempotency-*` commands require
`--create-snapshot`, which try-runtime v0.10.1 does not support ([P0 blocker](../../AI_MEMORIES/p0_blocker_try_runtime_snapshot.md)).
This means **idempotency testing is silently skipped** in CI — a critical gap
for the v0.9.40 → v1.12.0 upgrade with 30+ migrations.

## Strategy: Chopsticks Double-Fork

Chopsticks can fork live chain state at a specific block. We exploit this:

```
Live chain at block N
    │
    ├─→ Fork A: inject new WASM → produce 1 block → capture state root + pallet storage
    │
    └─→ Fork B: inject new WASM → produce 1 block → capture state root + pallet storage
                                                      │
                                                      ▼
                                            Compare A vs B
                                            (must be identical)
```

Both forks start from identical pre-migration state (block N). If migrations
are deterministic, post-migration state must be byte-identical.

## Prerequisites

- `bun` installed
- `@polkadot/api` and `@acala-network/chopsticks` packages
- WASM runtimes built (`cargo build --release`)
- Network access to archive node RPC endpoints

## Workflow

```
Idempotency Test Progress:
- [ ] Step 1: Pin block number for reproducibility
- [ ] Step 2: Fork A — inject WASM, produce block, capture state
- [ ] Step 3: Fork B — same block, same WASM, capture state
- [ ] Step 4: Compare state roots and key storage values
- [ ] Step 5: Report per-pallet determinism
```

### Quick start

```bash
# Single chain
bun run .claude/skills/testing-migration-idempotency/scripts/chopsticks-idempotency-test.ts \
  --chain leafchain-sand-testnet

# All testnet chains
bun run .claude/skills/testing-migration-idempotency/scripts/chopsticks-idempotency-test.ts \
  --chain all-testnet

# All chains (testnet + mainnet)
bun run .claude/skills/testing-migration-idempotency/scripts/chopsticks-idempotency-test.ts \
  --chain all
```

### What it compares

For each fork, after producing 1 post-migration block:

| Check | Method | Determinism signal |
|---|---|---|
| State root | `chain_getHeader` → `stateRoot` | Byte-identical = fully deterministic |
| specVersion | `state_getRuntimeVersion` | Must match (WASM applied) |
| Balances totalIssuance | `query.balances.totalIssuance` | Economic state preserved |
| System account count | `query.system.account.entries` count | No phantom accounts |
| Custom pallets | RWA, Crowdfunding, TrustlessAgent storage | Business data intact |

**State root comparison is the strongest signal** — if two independent forks
from the same pre-migration state produce the same state root after migration,
the migration is deterministic by definition.

## CI Integration

Add to `ci.yml` alongside the existing `chopsticks` job:

```yaml
chopsticks-idempotency:
  name: Chopsticks idempotency test
  needs: [preflight, build-binaries]
  runs-on: [self-hosted, hetzner-thxnet]
  timeout-minutes: 20
  if: # same branch filter as chopsticks job
  steps:
    - uses: actions/checkout@v5
    - name: Download WASM runtimes
      uses: actions/download-artifact@v5
      with:
        name: wasm-runtimes
        path: wasm
    - name: Setup
      run: |
        curl -fsSL https://bun.sh/install | bash
        export PATH="$HOME/.bun/bin:$PATH"
        bun install @polkadot/api @acala-network/chopsticks
        # Stage WASMs
        mkdir -p target/release/wbuild/general-runtime
        find wasm -name "general_runtime.compact.compressed.wasm" \
          -exec cp {} target/release/wbuild/general-runtime/ \;
    - name: Idempotency test (testnet chains)
      run: |
        export PATH="$HOME/.bun/bin:$PATH"
        bun run .claude/skills/testing-migration-idempotency/scripts/chopsticks-idempotency-test.ts \
          --chain all-testnet
      timeout-minutes: 15
```

## Limitations

- Chopsticks uses `mock-signature-host: true` — signature validation is skipped
- Chopsticks block production is single-threaded — no parachain consensus
- State root comparison cannot distinguish between "migration wrote same data"
  and "migration was a no-op" — verify specVersion changed to rule out no-op
- This is a **complement** to try-runtime idempotency (not a replacement);
  when try-runtime gains `--create-snapshot`, use both

## Relationship to Existing Tests

| Test | What it proves | Data source |
|---|---|---|
| `try-runtime test-all` | Migrations pass against live state | Live RPC |
| `try-runtime test-idempotency-*` | Migration determinism (BLOCKED) | Snapshot |
| `chopsticks upgrade-test.ts` | Chain produces blocks after upgrade | Live fork |
| **This skill** | Migration determinism via double-fork | Live fork x2 |
