#!/usr/bin/env bash
# =============================================================================
# Node binary DB migration test for THXNET.
# =============================================================================
#
# Tests the node-level database migration path: new binary + old chain data.
# This complements try-runtime (runtime state migration) by verifying the
# node-native DB layer (RocksDB schema, trie format, block body encoding).
#
# Usage:
#   ./scripts/db-migration-test.sh <chain> <new-binary> <chain-data-source>
#
#   chain:              rootchain-testnet | rootchain-mainnet |
#                       leafchain-sand-testnet | leafchain-avatect-mainnet |
#                       leafchain-lmt-testnet | leafchain-lmt-mainnet
#   new-binary:         Path to the new node binary
#   chain-data-source:  Path to existing chain data directory OR
#                       user@host:/path for rsync
#
# Environment:
#   RPC_PORT           Override RPC port (default: 29944)
#   P2P_PORT           Override P2P port (default: 30444)
#   STARTUP_WAIT       Seconds to wait for startup (default: 60)
#   SYNC_WAIT          Seconds to wait for block import (default: 180)
#   DRY_RUN            Set to 1 to print commands without executing
#
# Examples:
#   # Test with local chain data copy
#   ./scripts/db-migration-test.sh rootchain-testnet \
#     ./target/release/polkadot /tmp/chain-data/rootchain-testnet
#
#   # Test with remote chain data (rsync)
#   ./scripts/db-migration-test.sh rootchain-testnet \
#     ./target/release/polkadot archive-01:/data/rootchain-testnet
#
# =============================================================================

set -euo pipefail

# ─── Shared library ─────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../../_lib/rpc-helpers.sh"

# ─── Configuration ───────────────────────────────────────────────────────────

RPC_PORT="${RPC_PORT:-29944}"
P2P_PORT="${P2P_PORT:-30444}"
STARTUP_WAIT="${STARTUP_WAIT:-60}"
SYNC_WAIT="${SYNC_WAIT:-180}"
DRY_RUN="${DRY_RUN:-0}"
WORK_DIR="${WORK_DIR:-/tmp/db-migration-test}"

# ─── Aliases for backward-compatible function names ─────────────────────────

get_best_block_number()      { get_best_block; }
get_finalized_block_number() { get_finalized_block; }

# ─── Main ────────────────────────────────────────────────────────────────────

if [[ $# -lt 3 ]]; then
    echo "Usage: $0 <chain> <new-binary> <chain-data-source>"
    echo ""
    echo "  chain:             rootchain-testnet | rootchain-mainnet | leafchain-*"
    echo "  new-binary:        Path to the new node binary"
    echo "  chain-data-source: Local path or user@host:/path"
    exit 1
fi

CHAIN="$1"
NEW_BINARY="$2"
DATA_SOURCE="$3"

resolve_chain_params "${CHAIN}"

# Validate binary exists
if [[ ! -x "${NEW_BINARY}" ]]; then
    log_error "Binary not found or not executable: ${NEW_BINARY}"
    exit 1
fi

NODE_PID=""
cleanup() {
    if [[ -n "${NODE_PID}" ]]; then
        log_info "Stopping node (PID ${NODE_PID})..."
        kill "${NODE_PID}" 2>/dev/null || true
        wait "${NODE_PID}" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

PASSED=0
FAILED=0
TOTAL=0

check() {
    local name="$1"
    local condition="$2"
    local detail="$3"
    TOTAL=$((TOTAL + 1))
    if [[ "${condition}" == "true" ]]; then
        log_success "${name}: ${detail}"
        PASSED=$((PASSED + 1))
    else
        log_error "${name}: ${detail}"
        FAILED=$((FAILED + 1))
    fi
}

# ─── Step 1: Obtain chain data ──────────────────────────────────────────────

log_step "Step 1/6: Obtaining chain data"

LOCAL_DATA="${WORK_DIR}/${CHAIN}"
LOG_FILE="${WORK_DIR}/${CHAIN}-startup.log"
mkdir -p "${WORK_DIR}"

if [[ "${DATA_SOURCE}" == *:* ]]; then
    log_info "Rsyncing from ${DATA_SOURCE}..."
    if [[ "${DRY_RUN}" == "1" ]]; then
        log_warn "[DRY RUN] Would rsync ${DATA_SOURCE} -> ${LOCAL_DATA}"
    else
        rsync -avz --progress "${DATA_SOURCE}/" "${LOCAL_DATA}/"
    fi
else
    if [[ ! -d "${DATA_SOURCE}" ]]; then
        log_error "Chain data not found: ${DATA_SOURCE}"
        exit 1
    fi
    log_info "Copying ${DATA_SOURCE} -> ${LOCAL_DATA}..."
    if [[ "${DRY_RUN}" == "1" ]]; then
        log_warn "[DRY RUN] Would copy ${DATA_SOURCE} -> ${LOCAL_DATA}"
    else
        # Use cp -a to preserve all attributes; rsync for incremental
        if command -v rsync &>/dev/null; then
            rsync -a "${DATA_SOURCE}/" "${LOCAL_DATA}/"
        else
            cp -a "${DATA_SOURCE}" "${LOCAL_DATA}"
        fi
    fi
fi

DATA_SIZE=$(du -sh "${LOCAL_DATA}" 2>/dev/null | cut -f1 || echo "unknown")
log_info "Chain data size: ${DATA_SIZE}"

# ─── Step 2: Start new binary ───────────────────────────────────────────────

log_step "Step 2/6: Starting new binary against old chain data"

NODE_ARGS=()
if [[ "${BINARY_TYPE}" == "rootchain" ]]; then
    NODE_ARGS=(
        --chain "${CHAIN_SPEC}"
        --base-path "${LOCAL_DATA}"
        --no-hardware-benchmarks
        --rpc-port "${RPC_PORT}"
        --port "${P2P_PORT}"
        --no-prometheus
        --no-telemetry
    )
else
    NODE_ARGS=(
        --chain "${CHAIN_SPEC}"
        --base-path "${LOCAL_DATA}"
        --no-hardware-benchmarks
        --rpc-port "${RPC_PORT}"
        --port "${P2P_PORT}"
        --no-prometheus
        --no-telemetry
        --collator
        --
        --chain "${RELAY_CHAIN_SPEC}"
    )
fi

log_info "Binary:    ${NEW_BINARY}"
log_info "Chain:     ${CHAIN} (${CHAIN_SPEC})"
log_info "Data:      ${LOCAL_DATA}"
log_info "RPC port:  ${RPC_PORT}"
log_info "Args:      ${NODE_ARGS[*]}"

if [[ "${DRY_RUN}" == "1" ]]; then
    log_warn "[DRY RUN] Would start: ${NEW_BINARY} ${NODE_ARGS[*]}"
    log_warn "[DRY RUN] Skipping remaining steps"
    exit 0
fi

"${NEW_BINARY}" "${NODE_ARGS[@]}" 2>&1 | tee "${LOG_FILE}" &
NODE_PID=$!
log_info "Node started (PID ${NODE_PID})"

# ─── Step 3: Verify startup ─────────────────────────────────────────────────

log_step "Step 3/6: Verifying startup (waiting ${STARTUP_WAIT}s)"

sleep "${STARTUP_WAIT}"

# Check if process is still alive
if kill -0 "${NODE_PID}" 2>/dev/null; then
    check "node.alive" "true" "Node process is running after ${STARTUP_WAIT}s"
else
    check "node.alive" "false" "Node process died during startup"
    log_error "Startup log tail:"
    tail -50 "${LOG_FILE}" 2>/dev/null || true
    exit 1
fi

# Check for fatal errors in log
PANIC_COUNT=$(grep -ciE '(panic|fatal|incompatible.*database|corrupt)' "${LOG_FILE}" || true)
check "node.no_panics" "$( [[ ${PANIC_COUNT} -eq 0 ]] && echo true || echo false )" \
    "${PANIC_COUNT} fatal errors in startup log"

# Check for DB migration messages (informational)
DB_MIGRATION_LINES=$(grep -ciE '(database.*migrat|upgrading.*database|converting.*column|schema.*version)' "${LOG_FILE}" || true)
if [[ ${DB_MIGRATION_LINES} -gt 0 ]]; then
    log_info "DB migration activity detected (${DB_MIGRATION_LINES} log lines)"
    grep -iE '(database.*migrat|upgrading.*database|converting.*column|schema.*version)' "${LOG_FILE}" | head -10
fi

# ─── Step 4: Verify block import ────────────────────────────────────────────

log_step "Step 4/6: Verifying block import (waiting ${SYNC_WAIT}s)"

BLOCK_BEFORE=$(get_best_block_number)
log_info "Current best block: #${BLOCK_BEFORE}"

if [[ "${BLOCK_BEFORE}" -eq 0 ]]; then
    log_warn "Could not query best block — RPC may not be ready, waiting..."
    sleep 30
    BLOCK_BEFORE=$(get_best_block_number)
    log_info "Retry best block: #${BLOCK_BEFORE}"
fi

check "rpc.responsive" "$( [[ ${BLOCK_BEFORE} -gt 0 ]] && echo true || echo false )" \
    "Best block: #${BLOCK_BEFORE}"

sleep "${SYNC_WAIT}"

BLOCK_AFTER=$(get_best_block_number)
log_info "Best block after ${SYNC_WAIT}s: #${BLOCK_AFTER}"

BLOCKS_IMPORTED=$((BLOCK_AFTER - BLOCK_BEFORE))
check "blocks.importing" "$( [[ ${BLOCKS_IMPORTED} -gt 0 ]] && echo true || echo false )" \
    "Imported ${BLOCKS_IMPORTED} blocks (#${BLOCK_BEFORE} -> #${BLOCK_AFTER})"

# ─── Step 5: Verify state queries ───────────────────────────────────────────

log_step "Step 5/6: Verifying state queries"

# Runtime version
SPEC_VERSION=$(rpc_call "state_getRuntimeVersion" | jq -r '.result.specVersion // empty' 2>/dev/null || echo "")
check "state.runtime_version" "$( [[ -n "${SPEC_VERSION}" ]] && echo true || echo false )" \
    "specVersion: ${SPEC_VERSION:-unavailable}"

# System health
PEER_COUNT=$(rpc_call "system_health" | jq -r '.result.peers // 0' 2>/dev/null || echo "0")
IS_SYNCING=$(rpc_call "system_health" | jq -r '.result.isSyncing // empty' 2>/dev/null || echo "unknown")
log_info "Peers: ${PEER_COUNT}, Syncing: ${IS_SYNCING}"

# Total issuance (basic storage query — confirms trie is readable)
ISSUANCE_RESULT=$(rpc_call "state_getStorage" \
    "[\"0xc2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80\"]")
ISSUANCE_HEX=$(echo "${ISSUANCE_RESULT}" | jq -r '.result // empty' 2>/dev/null || echo "")
check "state.storage_query" "$( [[ -n "${ISSUANCE_HEX}" ]] && echo true || echo false )" \
    "totalIssuance storage: ${ISSUANCE_HEX:0:20}..."

# ─── Step 6: Verify finalization ─────────────────────────────────────────────

log_step "Step 6/6: Verifying finalization"

FINALIZED=$(get_finalized_block_number)
BEST=$(get_best_block_number)
GAP=$((BEST - FINALIZED))

log_info "Best: #${BEST}, Finalized: #${FINALIZED}, Gap: ${GAP}"

check "finalization.exists" "$( [[ ${FINALIZED} -gt 0 ]] && echo true || echo false )" \
    "Finalized block: #${FINALIZED}"

# Gap threshold: <100 for relay chain, <200 for parachain (slower finalization)
MAX_GAP=200
check "finalization.gap" "$( [[ ${GAP} -lt ${MAX_GAP} ]] && echo true || echo false )" \
    "Finalization gap: ${GAP} (threshold: ${MAX_GAP})"

# ─── Summary ─────────────────────────────────────────────────────────────────

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║              DB MIGRATION TEST RESULTS                      ║"
echo "╠══════════════════════════════════════════════════════════════╣"
printf "║  Chain:      %-45s ║\n" "${CHAIN}"
printf "║  Binary:     %-45s ║\n" "$(basename "${NEW_BINARY}")"
printf "║  Data size:  %-45s ║\n" "${DATA_SIZE}"
printf "║  Total:      %-45s ║\n" "${TOTAL} checks"
printf "║  Passed:     %-45s ║\n" "${PASSED}"
printf "║  Failed:     %-45s ║\n" "${FAILED}"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
log_info "Full startup log: ${LOG_FILE}"

exit "${FAILED}"
