#!/usr/bin/env bash
# =============================================================================
# Genesis-to-tip sync test for THXNET.
# =============================================================================
#
# Verifies the Chain Integrity Invariant: a freshly started node binary must
# sync from genesis to the chain tip without manual intervention.
#
# Usage:
#   ./genesis-sync-test.sh <chain> <binary> <sync-mode>
#   ./genesis-sync-test.sh monitor <rpc-port>
#
#   chain:      rootchain-testnet | rootchain-mainnet |
#               leafchain-sand-testnet | leafchain-avatect-mainnet |
#               leafchain-lmt-testnet | leafchain-lmt-mainnet
#   binary:     Path to node binary
#   sync-mode:  full | warp | fast (default: full)
#
# Environment:
#   RPC_PORT             Override RPC port (default: 29944)
#   P2P_PORT             Override P2P port (default: 30444)
#   BASE_PATH            Override chain data path (default: /tmp/genesis-sync-test/<chain>)
#   STUCK_THRESHOLD_MIN  Minutes without progress before FAIL (default: 10)
#   POLL_INTERVAL_SEC    Seconds between progress checks (default: 60)
#   MAX_RUNTIME_MIN      Maximum total runtime in minutes (default: 720 = 12h)
#   DRY_RUN              Set to 1 to print commands without executing
#   BOOTNODES            Space-separated list of bootnode addresses
#
# =============================================================================

set -euo pipefail

# ─── Shared library ─────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../../_lib/rpc-helpers.sh"

# ─── Configuration ───────────────────────────────────────────────────────────

RPC_PORT="${RPC_PORT:-29944}"
P2P_PORT="${P2P_PORT:-30444}"
STUCK_THRESHOLD_MIN="${STUCK_THRESHOLD_MIN:-10}"
POLL_INTERVAL_SEC="${POLL_INTERVAL_SEC:-60}"
MAX_RUNTIME_MIN="${MAX_RUNTIME_MIN:-720}"
DRY_RUN="${DRY_RUN:-0}"
BOOTNODES="${BOOTNODES:-}"

# ─── Monitor mode ────────────────────────────────────────────────────────────

monitor_sync() {
    local port="$1"
    RPC_PORT="${port}"

    log_step "Monitoring sync on port ${port}"

    local prev_best=0
    local stuck_count=0
    local start_time
    start_time=$(date +%s)

    while true; do
        local best peers syncing
        best=$(get_best_block)
        peers=$(get_peer_count)
        syncing=$(is_syncing)

        local elapsed=$(( ($(date +%s) - start_time) / 60 ))

        # Calculate import rate
        local rate="N/A"
        if [[ ${best} -gt 0 && ${elapsed} -gt 0 ]]; then
            rate="$(( best / elapsed )) blocks/min"
        fi

        echo -e "${BLUE}[${elapsed}m]${NC} Block: #${best} | Peers: ${peers} | Syncing: ${syncing} | Rate: ${rate}"

        # Detect stuck
        if [[ "${best}" == "${prev_best}" && ${best} -gt 0 ]]; then
            stuck_count=$((stuck_count + 1))
            if [[ ${stuck_count} -ge ${STUCK_THRESHOLD_MIN} ]]; then
                log_error "Sync stuck at #${best} for ${STUCK_THRESHOLD_MIN}+ minutes"
                return 1
            fi
        else
            stuck_count=0
        fi
        prev_best="${best}"

        # Detect completion
        if [[ "${syncing}" == "false" && ${best} -gt 0 ]]; then
            log_success "Sync complete at block #${best} (${elapsed} minutes)"
            return 0
        fi

        # Timeout
        if [[ ${elapsed} -ge ${MAX_RUNTIME_MIN} ]]; then
            log_error "Timeout: sync did not complete within ${MAX_RUNTIME_MIN} minutes (at block #${best})"
            return 1
        fi

        sleep "${POLL_INTERVAL_SEC}"
    done
}

# ─── Main ────────────────────────────────────────────────────────────────────

if [[ $# -lt 2 ]]; then
    echo "Usage:"
    echo "  $0 <chain> <binary> [sync-mode]    Start fresh sync test"
    echo "  $0 monitor <rpc-port>              Monitor existing sync"
    echo ""
    echo "Chains: rootchain-testnet, rootchain-mainnet, leafchain-sand-testnet,"
    echo "        leafchain-avatect-mainnet, leafchain-lmt-testnet, leafchain-lmt-mainnet"
    echo "Sync modes: full (default), warp, fast"
    exit 1
fi

# Handle monitor mode
if [[ "$1" == "monitor" ]]; then
    monitor_sync "$2"
    exit $?
fi

CHAIN="$1"
BINARY="$2"
SYNC_MODE="${3:-full}"

resolve_chain_params "${CHAIN}"

# Validate binary
if [[ ! -x "${BINARY}" ]]; then
    log_error "Binary not found or not executable: ${BINARY}"
    exit 1
fi

# Validate sync mode
case "${SYNC_MODE}" in
    full|warp|fast) ;;
    *)
        log_error "Invalid sync mode: ${SYNC_MODE} (valid: full, warp, fast)"
        exit 1
        ;;
esac

BASE_PATH="${BASE_PATH:-/tmp/genesis-sync-test/${CHAIN}}"
LOG_FILE="/tmp/genesis-sync-test/${CHAIN}-sync.log"

NODE_PID=""
cleanup() {
    if [[ -n "${NODE_PID}" ]]; then
        log_info "Stopping node (PID ${NODE_PID})..."
        kill "${NODE_PID}" 2>/dev/null || true
        wait "${NODE_PID}" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

# ─── Step 1: Clean environment ───────────────────────────────────────────────

log_step "Step 1: Preparing clean environment"

if [[ -d "${BASE_PATH}" ]]; then
    log_warn "Removing existing chain data at ${BASE_PATH}"
    rm -rf "${BASE_PATH}"
fi
mkdir -p "${BASE_PATH}" "$(dirname "${LOG_FILE}")"

log_info "Chain:     ${CHAIN} (${CHAIN_SPEC})"
log_info "Binary:    ${BINARY}"
log_info "Sync:      ${SYNC_MODE}"
log_info "Base path: ${BASE_PATH} (clean)"
log_info "Log:       ${LOG_FILE}"

# ─── Step 2: Start fresh node ────────────────────────────────────────────────

log_step "Step 2: Starting fresh node (--sync ${SYNC_MODE})"

NODE_ARGS=(
    --chain "${CHAIN_SPEC}"
    --base-path "${BASE_PATH}"
    --sync "${SYNC_MODE}"
    --state-pruning archive
    --no-hardware-benchmarks
    --rpc-port "${RPC_PORT}"
    --port "${P2P_PORT}"
    --rpc-cors all
    --no-prometheus
    --no-telemetry
    --log "sync=info,runtime=warn"
)

# Add bootnodes if specified
if [[ -n "${BOOTNODES}" ]]; then
    for bn in ${BOOTNODES}; do
        NODE_ARGS+=(--bootnodes "${bn}")
    done
fi

# Leafchain: add relay chain flags
if [[ "${BINARY_TYPE}" == "leafchain" ]]; then
    NODE_ARGS+=(
        --collator
        --
        --chain "${RELAY_CHAIN_SPEC}"
        --sync warp
    )
fi

if [[ "${DRY_RUN}" == "1" ]]; then
    log_warn "[DRY RUN] Would start: ${BINARY} ${NODE_ARGS[*]}"
    exit 0
fi

"${BINARY}" "${NODE_ARGS[@]}" 2>&1 | tee "${LOG_FILE}" &
NODE_PID=$!
log_info "Node started (PID ${NODE_PID})"

# Wait for RPC to be ready
log_info "Waiting for RPC to become available..."
for i in $(seq 1 30); do
    if get_best_block > /dev/null 2>&1; then
        break
    fi
    sleep 2
done

# ─── Step 3: Monitor sync ───────────────────────────────────────────────────

log_step "Step 3: Monitoring sync progress"

SYNC_START=$(date +%s)
PREV_BEST=0
STUCK_COUNT=0

while true; do
    BEST=$(get_best_block)
    PEERS=$(get_peer_count)
    SYNCING=$(is_syncing)

    ELAPSED_MIN=$(( ($(date +%s) - SYNC_START) / 60 ))

    # Rate calculation
    RATE="N/A"
    if [[ ${BEST} -gt 0 && ${ELAPSED_MIN} -gt 0 ]]; then
        RATE="$(( BEST / ELAPSED_MIN )) blocks/min"
    fi

    log_info "Block: #${BEST} | Peers: ${PEERS} | Syncing: ${SYNCING} | Rate: ${RATE} | ${ELAPSED_MIN}m elapsed"

    # Check process alive
    if ! kill -0 "${NODE_PID}" 2>/dev/null; then
        log_error "Node process died at block #${BEST}"
        log_error "Last 30 lines of log:"
        tail -30 "${LOG_FILE}" 2>/dev/null || true
        exit 1
    fi

    # Detect stuck
    if [[ "${BEST}" == "${PREV_BEST}" && ${BEST} -gt 0 && ${PEERS} -gt 0 ]]; then
        STUCK_COUNT=$((STUCK_COUNT + 1))
        if [[ ${STUCK_COUNT} -ge ${STUCK_THRESHOLD_MIN} ]]; then
            log_error "Sync stuck at #${BEST} for ${STUCK_THRESHOLD_MIN}+ minutes (${PEERS} peers available)"
            log_error "Possible causes: BadBlock, missing host function, state decode error"
            log_error "Last 30 lines of log:"
            tail -30 "${LOG_FILE}" 2>/dev/null || true
            exit 1
        fi
    else
        STUCK_COUNT=0
    fi
    PREV_BEST="${BEST}"

    # Detect completion
    if [[ "${SYNCING}" == "false" && ${BEST} -gt 0 ]]; then
        log_success "Sync complete at block #${BEST} (${ELAPSED_MIN} minutes)"
        break
    fi

    # Timeout
    if [[ ${ELAPSED_MIN} -ge ${MAX_RUNTIME_MIN} ]]; then
        log_error "Timeout: ${MAX_RUNTIME_MIN} minutes exceeded (at block #${BEST})"
        exit 1
    fi

    sleep "${POLL_INTERVAL_SEC}"
done

# ─── Step 4: Post-sync health checks ────────────────────────────────────────

log_step "Step 4: Post-sync health checks"

PASSED=0
FAILED=0

check() {
    local name="$1" ok="$2" detail="$3"
    if [[ "${ok}" == "true" ]]; then
        log_success "${name}: ${detail}"
        PASSED=$((PASSED + 1))
    else
        log_error "${name}: ${detail}"
        FAILED=$((FAILED + 1))
    fi
}

# Finalization
FINALIZED=$(get_finalized_block)
BEST=$(get_best_block)
FIN_GAP=$((BEST - FINALIZED))
check "finalization" "$( [[ ${FINALIZED} -gt 0 ]] && echo true || echo false )" \
    "Finalized: #${FINALIZED}, Best: #${BEST}, Gap: ${FIN_GAP}"

# Runtime version queryable
SPEC=$(rpc_call "state_getRuntimeVersion" | jq -r '.result.specVersion // empty' 2>/dev/null)
check "runtime_version" "$( [[ -n "${SPEC}" ]] && echo true || echo false )" \
    "specVersion: ${SPEC:-unavailable}"

# State storage queryable
ISSUANCE=$(rpc_call "state_getStorage" \
    "[\"0xc2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80\"]" \
    | jq -r '.result // empty' 2>/dev/null)
check "state_query" "$( [[ -n "${ISSUANCE}" ]] && echo true || echo false )" \
    "totalIssuance: ${ISSUANCE:0:20}..."

# Peer count
FINAL_PEERS=$(get_peer_count)
check "peers" "$( [[ ${FINAL_PEERS} -gt 0 ]] && echo true || echo false )" \
    "Connected peers: ${FINAL_PEERS}"

# Check for panics in entire log
PANIC_COUNT=$(grep -ciE '(panic|fatal)' "${LOG_FILE}" || true)
check "no_panics" "$( [[ ${PANIC_COUNT} -eq 0 ]] && echo true || echo false )" \
    "${PANIC_COUNT} panics in sync log"

# ─── Summary ─────────────────────────────────────────────────────────────────

TOTAL_ELAPSED=$(( ($(date +%s) - SYNC_START) / 60 ))
TOTAL=$((PASSED + FAILED))

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║            GENESIS-TO-TIP SYNC TEST RESULTS                 ║"
echo "╠══════════════════════════════════════════════════════════════╣"
printf "║  Chain:        %-43s ║\n" "${CHAIN}"
printf "║  Sync mode:    %-43s ║\n" "${SYNC_MODE}"
printf "║  Final block:  %-43s ║\n" "#${BEST}"
printf "║  Duration:     %-43s ║\n" "${TOTAL_ELAPSED} minutes"
printf "║  Checks:       %-43s ║\n" "${PASSED}/${TOTAL} passed"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
log_info "Full sync log: ${LOG_FILE}"

# Disk usage report
DATA_SIZE=$(du -sh "${BASE_PATH}" 2>/dev/null | cut -f1 || echo "unknown")
log_info "Chain data size after sync: ${DATA_SIZE}"

exit "${FAILED}"
