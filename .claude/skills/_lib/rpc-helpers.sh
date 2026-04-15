#!/usr/bin/env bash
# =============================================================================
# Shared helpers for THXNET. node testing scripts
# =============================================================================
#
# Source this file from test scripts:
#   SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
#   source "${SCRIPT_DIR}/../../_lib/rpc-helpers.sh"
#
# Provides:
#   - Color constants (RED, GREEN, YELLOW, BLUE, CYAN, NC)
#   - Log functions (log_info, log_success, log_warn, log_error, log_step)
#   - RPC helpers (rpc_call, get_best_block, get_finalized_block,
#                  get_peer_count, is_syncing)
#   - Chain parameter resolution (resolve_chain_params)
#
# Requires:
#   - RPC_PORT must be set before calling RPC helpers (default: 29944)
#   - curl and jq on PATH
#
# =============================================================================

# Guard against double-sourcing
if [[ -n "${_RPC_HELPERS_LOADED:-}" ]]; then
    return 0
fi
readonly _RPC_HELPERS_LOADED=1

# ─── Colors ──────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# ─── Log functions ───────────────────────────────────────────────────────────

log_info()    { echo -e "${BLUE}[INFO]${NC} $(date '+%H:%M:%S') $*"; }
log_success() { echo -e "${GREEN}[PASS]${NC} $(date '+%H:%M:%S') $*"; }
log_warn()    { echo -e "${YELLOW}[WARN]${NC} $(date '+%H:%M:%S') $*"; }
log_error()   { echo -e "${RED}[FAIL]${NC} $(date '+%H:%M:%S') $*"; }
log_step()    { echo -e "\n${CYAN}════════════════════════════════════════════════════════════${NC}"; \
                echo -e "${CYAN}  $*${NC}"; \
                echo -e "${CYAN}════════════════════════════════════════════════════════════${NC}\n"; }

# ─── RPC helpers ─────────────────────────────────────────────────────────────

rpc_call() {
    local method="$1"
    local params="${2:-[]}"
    curl -s --max-time 10 \
        -H "Content-Type: application/json" \
        -d "{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"${method}\",\"params\":${params}}" \
        "http://localhost:${RPC_PORT}" 2>/dev/null
}

get_best_block() {
    local hex
    hex=$(rpc_call "chain_getHeader" | jq -r '.result.number // "0x0"' 2>/dev/null)
    printf "%d\n" "${hex}" 2>/dev/null || echo "0"
}

get_finalized_block() {
    local hash
    hash=$(rpc_call "chain_getFinalizedHead" | jq -r '.result // empty' 2>/dev/null)
    if [[ -z "${hash}" ]]; then echo "0"; return; fi
    local hex
    hex=$(rpc_call "chain_getHeader" "[\"${hash}\"]" | jq -r '.result.number // "0x0"' 2>/dev/null)
    printf "%d\n" "${hex}" 2>/dev/null || echo "0"
}

get_peer_count() {
    rpc_call "system_health" | jq -r '.result.peers // 0' 2>/dev/null || echo "0"
}

is_syncing() {
    rpc_call "system_health" | jq -r '.result.isSyncing // "unknown"' 2>/dev/null || echo "unknown"
}

# ─── Chain parameter resolution ──────────────────────────────────────────────
#
# Sets: BINARY_TYPE, CHAIN_SPEC, and (for leafchains) RELAY_CHAIN_SPEC
#

resolve_chain_params() {
    local chain="$1"
    case "${chain}" in
        rootchain-testnet)
            BINARY_TYPE="rootchain"
            CHAIN_SPEC="thxnet-testnet"
            ;;
        rootchain-mainnet)
            BINARY_TYPE="rootchain"
            CHAIN_SPEC="thxnet"
            ;;
        leafchain-sand-testnet)
            BINARY_TYPE="leafchain"
            CHAIN_SPEC="leafchain-sand-testnet"
            RELAY_CHAIN_SPEC="thxnet-testnet"
            ;;
        leafchain-avatect-mainnet)
            BINARY_TYPE="leafchain"
            CHAIN_SPEC="leafchain-avatect-mainnet"
            RELAY_CHAIN_SPEC="thxnet"
            ;;
        leafchain-lmt-testnet)
            BINARY_TYPE="leafchain"
            CHAIN_SPEC="leafchain-lmt-testnet"
            RELAY_CHAIN_SPEC="thxnet-testnet"
            ;;
        leafchain-lmt-mainnet)
            BINARY_TYPE="leafchain"
            CHAIN_SPEC="leafchain-lmt-mainnet"
            RELAY_CHAIN_SPEC="thxnet"
            ;;
        *)
            log_error "Unknown chain: ${chain}"
            echo "Valid: rootchain-testnet, rootchain-mainnet, leafchain-sand-testnet,"
            echo "       leafchain-avatect-mainnet, leafchain-lmt-testnet, leafchain-lmt-mainnet"
            exit 1
            ;;
    esac
}
