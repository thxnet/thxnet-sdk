---
name: testing-node-db-migration
description: >
  Tests node binary-level database migration by starting the new binary against
  existing chain data from the old version. Verifies RocksDB schema compatibility,
  block import continuity, and state query correctness after binary swap.
  Complements try-runtime (which only tests runtime state migration) by covering
  the node-native DB layer that try-runtime cannot reach.
  Use when: db migration, binary upgrade test, database compatibility, node upgrade,
  chain data migration, RocksDB migration, binary swap test, old data new binary,
  node-level migration, DB schema upgrade, 資料庫遷移測試, 節點二進制升級.
---

# Node Binary DB Migration Testing

## Problem

`try-runtime on-runtime-upgrade` validates **runtime state migrations** (pallet StorageVersion bumps, storage key transformations). It does NOT test:

- RocksDB / ParityDB column family schema changes between Substrate versions
- Trie format compatibility (state trie version changes)
- Block body storage format changes
- Off-chain storage / indexing DB format changes
- The node binary's own startup-time DB migration logic (`sc_client_db`)

When upgrading from v0.9.40 to v1.12.0, these node-level DB changes can cause the binary to refuse to start, panic during block import, or silently corrupt state.

## Strategy

```
Old binary + old chain data  (production-like state)
         │
         ▼
New binary + old chain data  (the actual upgrade path)
         │
         ▼
Verify: starts? imports blocks? queries work? finalizes?
```

## Prerequisites

- Access to chain data from a running node (rsync/scp from a non-validator archive node)
- Both old and new node binaries (old = currently deployed, new = candidate)
- Sufficient disk space (chain data can be 50-200+ GB per chain)

## Workflow

Copy this checklist and track progress:

```
DB Migration Test Progress:
- [ ] Step 1: Obtain chain data snapshot
- [ ] Step 2: Start new binary against old chain data
- [ ] Step 3: Verify startup and DB migration logs
- [ ] Step 4: Verify block import continuity
- [ ] Step 5: Verify state queries
- [ ] Step 6: Verify finalization
```

### Step 1: Obtain chain data snapshot

```bash
# Option A: rsync from archive node (recommended for production testing)
CHAIN_NAME="rootchain-testnet"  # or any target chain
REMOTE_HOST="archive-node-host"
REMOTE_DATA="/data/${CHAIN_NAME}"
LOCAL_DATA="/tmp/db-migration-test/${CHAIN_NAME}"

mkdir -p "${LOCAL_DATA}"
rsync -avz --progress "${REMOTE_HOST}:${REMOTE_DATA}/" "${LOCAL_DATA}/"

# Option B: Use existing local node data (copy, never test on original)
cp -a "/data/${CHAIN_NAME}" "${LOCAL_DATA}"
```

**Critical**: Always work on a COPY. Never point a test binary at production data.

### Step 2: Start new binary against old chain data

```bash
NEW_BINARY="./target/release/polkadot"  # or thxnet-leafchain
LOG_FILE="/tmp/db-migration-test/${CHAIN_NAME}-startup.log"

# For rootchain:
"${NEW_BINARY}" \
  --chain thxnet \
  --base-path "${LOCAL_DATA}" \
  --no-hardware-benchmarks \
  --sync full \
  --rpc-port 29944 \
  --port 30444 \
  2>&1 | tee "${LOG_FILE}" &

NODE_PID=$!
echo "Node PID: ${NODE_PID}"
```

For leafchain, add `--collator` and relay chain arguments as appropriate.

### Step 3: Verify startup and DB migration logs

Watch the first 60 seconds of logs:

```bash
# Check for DB migration messages
grep -iE '(database|migration|upgrade|schema|version|opening|converting)' "${LOG_FILE}"

# Check for errors or panics
grep -iE '(error|panic|fatal|incompatible|corrupt)' "${LOG_FILE}"
```

**PASS criteria**:
- No panics or fatal errors in the first 60 seconds
- If DB migration messages appear, they should complete successfully
- The node proceeds to block import phase

**FAIL indicators**:
- `Incompatible database version` — DB schema gap between versions
- `Database corrupted` — trie format incompatibility
- Any panic during startup

### Step 4: Verify block import continuity

```bash
# Query current block via RPC (wait for node to be ready)
sleep 30
curl -s -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"chain_getHeader","params":[]}' \
  http://localhost:29944 | jq '.result.number' | xargs printf "%d\n"

# Wait 2 minutes, query again — block number should increase
sleep 120
curl -s -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"chain_getHeader","params":[]}' \
  http://localhost:29944 | jq '.result.number' | xargs printf "%d\n"
```

**PASS**: Block number increases (node is importing new blocks).

### Step 5: Verify state queries

```bash
# System account count
curl -s -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"state_getStorage","params":["0x26aa394eea5630e07c48ae0c9558cef7b99d880ec681799c0cf30e8886371da9"]}' \
  http://localhost:29944 | jq '.result'

# Runtime version
curl -s -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"state_getRuntimeVersion","params":[]}' \
  http://localhost:29944 | jq '.result.specVersion'
```

**PASS**: Queries return valid data without errors.

### Step 6: Verify finalization

```bash
# Check finalized block
curl -s -H "Content-Type: application/json" \
  -d '{"id":1,"jsonrpc":"2.0","method":"chain_getFinalizedHead","params":[]}' \
  http://localhost:29944 | jq '.result'

# Compare finalized vs best — gap should be small and closing
BEST=$(curl -s -d '{"id":1,"jsonrpc":"2.0","method":"chain_getHeader","params":[]}' \
  -H "Content-Type: application/json" http://localhost:29944 | jq -r '.result.number' | xargs printf "%d\n")

FINALIZED_HASH=$(curl -s -d '{"id":1,"jsonrpc":"2.0","method":"chain_getFinalizedHead","params":[]}' \
  -H "Content-Type: application/json" http://localhost:29944 | jq -r '.result')

FINALIZED=$(curl -s -d "{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"chain_getHeader\",\"params\":[\"${FINALIZED_HASH}\"]}" \
  -H "Content-Type: application/json" http://localhost:29944 | jq -r '.result.number' | xargs printf "%d\n")

echo "Best: ${BEST}, Finalized: ${FINALIZED}, Gap: $((BEST - FINALIZED))"
```

**PASS**: Finalized block exists and the gap to best block is small (< 100 for relay chain).

### Cleanup

```bash
kill "${NODE_PID}" 2>/dev/null
rm -rf "${LOCAL_DATA}"  # Remove the copy
```

## CI Integration

This test is **too heavy for per-commit CI** (requires real chain data, 50-200+ GB). Recommended integration:

| Trigger | Scope | Chains |
|---|---|---|
| Nightly cron (upgrade branches) | Full test | rootchain-testnet, 1 leafchain-testnet |
| Pre-release gate (manual) | Full test | All 6 CI-covered chains |
| Per-commit CI | Skip | N/A (use try-runtime for runtime-level coverage) |

See [scripts/db-migration-test.sh](scripts/db-migration-test.sh) for the automated script.

## Chain-Specific Notes

| Chain Type | Binary | Extra Args |
|---|---|---|
| Rootchain (mainnet) | `polkadot` | `--chain thxnet` |
| Rootchain (testnet) | `polkadot` | `--chain thxnet-testnet` |
| Leafchain (any) | `thxnet-leafchain` | `--chain <chain-spec> --collator -- --chain thxnet` |

## What This Catches That try-runtime Misses

| Failure Mode | try-runtime | This test |
|---|---|---|
| Runtime storage migration bug | YES | NO (not its job) |
| RocksDB column family schema change | NO | YES |
| Trie version incompatibility | NO | YES |
| Block body decode failure | NO | YES |
| Offchain DB format change | NO | YES |
| Node startup panic | NO | YES |
| P2P protocol version mismatch | NO | Partial (observes peer count) |
