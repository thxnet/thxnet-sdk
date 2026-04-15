---
name: testing-genesis-to-tip-sync
description: >
  Verifies the Chain Integrity Invariant: a freshly started node binary must sync
  from genesis (block 0) to the chain tip without manual intervention. Starts a
  clean node with no existing chain data, syncs against the live network, and
  monitors progress until tip is reached or failure is detected. Catches historical
  block import failures, embedded runtime upgrade incompatibilities, and state
  transition errors that try-runtime and Chopsticks cannot surface.
  Use when: genesis sync, full sync test, chain integrity, genesis to tip,
  sync from scratch, fresh node sync, block import test, historical blocks,
  chain integrity invariant, 從創世塊同步, 全同步測試, 鏈完整性.
---

# Genesis-to-Tip Sync Testing

## Problem

The Chain Integrity Invariant (`.claude/CLAUDE.md`) requires:

> A freshly started node binary must sync from genesis to the latest block
> without manual intervention, with all features working as expected.

Neither try-runtime nor Chopsticks test this. They operate on a **single
state snapshot** (the current tip). They never execute the historical block
import path where:

- Block N was produced by runtime v1, but the current binary must re-execute it
- A runtime upgrade at block M embedded a WASM blob — the binary must apply it
- State transitions accumulate over millions of blocks — any single decode
  failure aborts the sync

## What This Catches

| Failure mode | try-runtime | Chopsticks | **This test** |
|---|---|---|---|
| Historical block import decode error | NO | NO | YES |
| Embedded runtime upgrade at block M fails | NO | NO | YES |
| State accumulation overflow/corruption | NO | NO | YES |
| Missing host function for old runtime | NO | NO | YES |
| DB growth exceeds disk during sync | NO | NO | YES |
| P2P bootstrap / peer discovery failure | NO | NO | YES |

## Strategy

```
Fresh node binary (no chain data, no DB)
         │
         ├─ Connect to bootnode peers
         ├─ Import blocks 0 → N (full execution mode)
         ├─ Monitor: block height, import rate, errors
         │
         ▼
   Reach chain tip? ──→ PASS
   Stuck or error?  ──→ FAIL (report block height + error)
```

### Sync modes

| Mode | Speed | Coverage | Use case |
|---|---|---|---|
| `--sync full` | Slow (hours-days) | Every block fully executed | Definitive proof |
| `--sync warp` | Fast (minutes) | Warp to tip, verify recent blocks | Quick sanity check |
| `--sync fast` | Medium | Download state, verify recent blocks | Compromise |

**For Chain Integrity Invariant: `--sync full` is required.** Warp sync skips
historical block execution and cannot prove the invariant.

## Prerequisites

- New node binary (`polkadot` or `thxnet-leafchain`)
- Network access to live chain peers (or explicit bootnodes)
- Sufficient disk space (estimate: 100-500 GB per chain for full sync)
- Time budget: full sync of a mature chain can take hours to days

## Workflow

```
Genesis Sync Test Progress:
- [ ] Step 1: Prepare clean environment
- [ ] Step 2: Start fresh node in full sync mode
- [ ] Step 3: Monitor sync progress
- [ ] Step 4: Detect completion or failure
- [ ] Step 5: Post-sync health checks
```

### Quick start

```bash
# Single chain — full sync (definitive, slow)
.claude/skills/testing-genesis-to-tip-sync/scripts/genesis-sync-test.sh \
  rootchain-testnet ./target/release/polkadot full

# Single chain — warp sync (quick sanity check)
.claude/skills/testing-genesis-to-tip-sync/scripts/genesis-sync-test.sh \
  rootchain-testnet ./target/release/polkadot warp

# Monitor an already-running sync
.claude/skills/testing-genesis-to-tip-sync/scripts/genesis-sync-test.sh \
  monitor 29944
```

### Step 1: Prepare clean environment

```bash
CHAIN="rootchain-testnet"
BINARY="./target/release/polkadot"
BASE_PATH="/tmp/genesis-sync-test/${CHAIN}"

# Ensure NO existing chain data
rm -rf "${BASE_PATH}"
mkdir -p "${BASE_PATH}"
```

### Step 2: Start fresh node

```bash
# Rootchain example
"${BINARY}" \
  --chain thxnet-testnet \
  --base-path "${BASE_PATH}" \
  --sync full \
  --no-hardware-benchmarks \
  --rpc-port 29944 \
  --port 30444 \
  --rpc-cors all \
  --no-prometheus \
  --no-telemetry \
  --log sync=info,runtime=warn \
  2>&1 | tee "/tmp/genesis-sync-test/${CHAIN}-sync.log" &

NODE_PID=$!
```

For leafchain, add relay chain flags:
```bash
"${BINARY}" \
  --base-path "${BASE_PATH}" \
  --chain leafchain-sand-testnet \
  --sync full \
  --rpc-port 29944 \
  --collator \
  -- \
  --chain thxnet-testnet \
  --sync warp
```

### Step 3: Monitor sync progress

```bash
# Poll every 60 seconds
while true; do
  BEST=$(curl -s -d '{"id":1,"jsonrpc":"2.0","method":"chain_getHeader","params":[]}' \
    -H "Content-Type: application/json" http://localhost:29944 \
    | jq -r '.result.number // "0x0"' | xargs printf "%d\n" 2>/dev/null)

  PEERS=$(curl -s -d '{"id":1,"jsonrpc":"2.0","method":"system_health","params":[]}' \
    -H "Content-Type: application/json" http://localhost:29944 \
    | jq -r '.result.peers // 0')

  SYNCING=$(curl -s -d '{"id":1,"jsonrpc":"2.0","method":"system_health","params":[]}' \
    -H "Content-Type: application/json" http://localhost:29944 \
    | jq -r '.result.isSyncing // "unknown"')

  echo "[$(date '+%H:%M:%S')] Block: #${BEST}, Peers: ${PEERS}, Syncing: ${SYNCING}"

  # Detect stuck sync (no progress in 10 minutes)
  if [[ "${BEST}" == "${PREV_BEST:-}" ]]; then
    STUCK_COUNT=$((${STUCK_COUNT:-0} + 1))
    if [[ ${STUCK_COUNT} -ge 10 ]]; then
      echo "FAIL: Sync stuck at #${BEST} for 10+ minutes"
      break
    fi
  else
    STUCK_COUNT=0
  fi
  PREV_BEST="${BEST}"

  # Detect completion
  if [[ "${SYNCING}" == "false" && ${BEST} -gt 0 ]]; then
    echo "Sync complete at #${BEST}"
    break
  fi

  sleep 60
done
```

### Step 4: Detect completion or failure

**PASS criteria**:
- `isSyncing` transitions from `true` to `false`
- Block height matches or is near the live chain tip
- No panics or fatal errors in the log

**FAIL indicators**:
- Node panics during block import → check log for the failing block number
- Sync stalls (no progress for 10+ minutes with peers available)
- `BadBlock` error → specific block fails execution
- `RuntimeApiError` → missing host function for historical runtime

### Step 5: Post-sync health checks

```bash
# Verify finalization is working
FINALIZED_HASH=$(curl -s -d '{"id":1,"jsonrpc":"2.0","method":"chain_getFinalizedHead","params":[]}' \
  -H "Content-Type: application/json" http://localhost:29944 | jq -r '.result')

# Verify state is queryable at the tip
curl -s -d '{"id":1,"jsonrpc":"2.0","method":"state_getRuntimeVersion","params":[]}' \
  -H "Content-Type: application/json" http://localhost:29944 | jq '.result.specVersion'

# Verify balances (basic state integrity)
curl -s -d '{"id":1,"jsonrpc":"2.0","method":"state_getStorage","params":["0xc2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80"]}' \
  -H "Content-Type: application/json" http://localhost:29944 | jq '.result'
```

## CI Integration

Full genesis sync is too slow for per-commit CI. Recommended schedule:

| Trigger | Mode | Chains | Estimated time |
|---|---|---|---|
| Weekly cron | `--sync full` | rootchain-testnet | Hours (depends on chain height) |
| Nightly (release branches) | `--sync warp` | All 3 testnet chains | ~15 min total |
| Pre-release gate | `--sync full` | rootchain-testnet + 1 leafchain | Hours |
| Per-commit CI | Skip | N/A | Use try-runtime instead |

### Cron job example

```yaml
name: Genesis Sync Test (Weekly)

on:
  schedule:
    - cron: '0 2 * * 0'  # Sunday 2am UTC
  workflow_dispatch:

jobs:
  genesis-sync:
    runs-on: [self-hosted, hetzner-thxnet]
    timeout-minutes: 720  # 12 hours
    steps:
      - uses: actions/checkout@v5
      - name: Build binary
        run: cargo build --locked --release -p polkadot
      - name: Genesis sync test (rootchain-testnet)
        run: |
          .claude/skills/testing-genesis-to-tip-sync/scripts/genesis-sync-test.sh \
            rootchain-testnet ./target/release/polkadot full
        timeout-minutes: 600
```

## Testing Priority Order

When time is limited, test chains in this order (highest risk first):

1. **rootchain-testnet** — most blocks, most runtime upgrades in history
2. **leafchain-sand-testnet** — oldest leafchain, most accumulated state
3. **rootchain-mainnet** — production, but similar to testnet
4. **leafchain-lmt-testnet** — newer chain, shorter history
5. **leafchain-avatect-mainnet** — newer, less risk
6. **leafchain-lmt-mainnet** — newest, least risk

## Relationship to Other Tests

```
          try-runtime          Chopsticks         Genesis Sync
              │                    │                   │
              ▼                    ▼                   ▼
        Runtime state         Forked state        Full historical
        migration at          + block prod        block import
        current tip           at current tip      from block 0
              │                    │                   │
              └────────┬───────────┘                   │
                       │                               │
              Current state is valid          ALL historical states
              after migration                 were valid in sequence
```

All three are needed. No single test covers the full upgrade path.
