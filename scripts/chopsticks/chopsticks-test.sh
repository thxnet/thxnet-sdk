#!/usr/bin/env bash
# Chopsticks upgrade test orchestrator
# Runs upgrade-test.ts and post-upgrade-pallet-test.ts against each chain config.
#
# Usage:
#   ./scripts/chopsticks/chopsticks-test.sh [chain]
#   chain: leafchain-sand-testnet (default), leafchain-avatect-mainnet, leafchain-lmt-testnet, leafchain-lmt-mainnet, leafchain-ecq-testnet, leafchain-ecq-mainnet, rootchain-testnet, rootchain-mainnet, all
#
# Prerequisites:
#   - bun installed
#   - WASM runtimes built: cargo build --release -p polkadot -p thxnet-leafchain

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
WASM_DIR="${CARGO_TARGET_DIR:-${PROJECT_ROOT}/target}/release/wbuild"

# WASM paths
ROOTCHAIN_MAINNET_WASM="${WASM_DIR}/thxnet-runtime/thxnet_runtime.compact.compressed.wasm"
ROOTCHAIN_TESTNET_WASM="${WASM_DIR}/thxnet-testnet-runtime/thxnet_testnet_runtime.compact.compressed.wasm"
LEAFCHAIN_WASM="${WASM_DIR}/general-runtime/general_runtime.compact.compressed.wasm"

# Chain configs: name -> config_file, wasm, port
declare -A CHAIN_CONFIG CHAIN_WASM CHAIN_PORT
CHAIN_CONFIG[leafchain-sand-testnet]="${SCRIPT_DIR}/leafchain-sand-testnet.yml"
CHAIN_CONFIG[leafchain-avatect-mainnet]="${SCRIPT_DIR}/leafchain-avatect-mainnet.yml"
CHAIN_CONFIG[leafchain-lmt-testnet]="${SCRIPT_DIR}/leafchain-lmt-testnet.yml"
CHAIN_CONFIG[leafchain-lmt-mainnet]="${SCRIPT_DIR}/leafchain-lmt-mainnet.yml"
CHAIN_CONFIG[leafchain-ecq-testnet]="${SCRIPT_DIR}/leafchain-ecq-testnet.yml"
CHAIN_CONFIG[leafchain-ecq-mainnet]="${SCRIPT_DIR}/leafchain-ecq-mainnet.yml"
CHAIN_CONFIG[rootchain-testnet]="${SCRIPT_DIR}/rootchain-testnet.yml"
CHAIN_CONFIG[rootchain-mainnet]="${SCRIPT_DIR}/rootchain-mainnet.yml"

CHAIN_WASM[leafchain-sand-testnet]="${LEAFCHAIN_WASM}"
CHAIN_WASM[leafchain-avatect-mainnet]="${LEAFCHAIN_WASM}"
CHAIN_WASM[leafchain-lmt-testnet]="${LEAFCHAIN_WASM}"
CHAIN_WASM[leafchain-lmt-mainnet]="${LEAFCHAIN_WASM}"
CHAIN_WASM[leafchain-ecq-testnet]="${LEAFCHAIN_WASM}"
CHAIN_WASM[leafchain-ecq-mainnet]="${LEAFCHAIN_WASM}"
CHAIN_WASM[rootchain-testnet]="${ROOTCHAIN_TESTNET_WASM}"
CHAIN_WASM[rootchain-mainnet]="${ROOTCHAIN_MAINNET_WASM}"

CHAIN_PORT[leafchain-sand-testnet]=8102
CHAIN_PORT[leafchain-avatect-mainnet]=8103
CHAIN_PORT[leafchain-lmt-testnet]=8104
CHAIN_PORT[leafchain-lmt-mainnet]=8105
CHAIN_PORT[leafchain-ecq-testnet]=8106
CHAIN_PORT[leafchain-ecq-mainnet]=8107
CHAIN_PORT[rootchain-testnet]=8100
CHAIN_PORT[rootchain-mainnet]=8101

# Leafchains have RWA/CF pallets; rootchains have DAO
LEAFCHAINS="leafchain-sand-testnet leafchain-avatect-mainnet leafchain-lmt-testnet leafchain-lmt-mainnet leafchain-ecq-testnet leafchain-ecq-mainnet"

CHAIN="${1:-leafchain-sand-testnet}"
PASSED=0
FAILED=0
TOTAL=0

cleanup() {
    pkill -f "chopsticks.*${SCRIPT_DIR}" 2>/dev/null || true
}
trap cleanup EXIT

run_chain_test() {
    local chain="$1"
    local config="${CHAIN_CONFIG[$chain]}"
    local wasm="${CHAIN_WASM[$chain]}"
    local port="${CHAIN_PORT[$chain]}"

    echo ""
    echo "═══════════════════════════════════════════════"
    echo "  Testing: ${chain} (port ${port})"
    echo "═══════════════════════════════════════════════"

    # Check WASM exists
    if [[ ! -f "${wasm}" ]]; then
        echo "SKIP: WASM not found: ${wasm}"
        return
    fi

    # Start Chopsticks
    echo "Starting Chopsticks..."
    bunx @acala-network/chopsticks -c "${config}" -w "${wasm}" >/dev/null 2>&1 &
    local chopsticks_pid=$!
    sleep 15

    if ! kill -0 "${chopsticks_pid}" 2>/dev/null; then
        echo "FAIL: Chopsticks failed to start"
        ((FAILED++)) || true
        ((TOTAL++)) || true
        return
    fi

    # Run upgrade test
    echo "--- upgrade-test.ts ---"
    ((TOTAL++)) || true
    if bun run "${SCRIPT_DIR}/upgrade-test.ts" --port "${port}" 2>&1; then
        ((PASSED++)) || true
    else
        ((FAILED++)) || true
    fi

    # Run pallet test (only for leafchains — rootchain has different pallets)
    if [[ "${LEAFCHAINS}" == *"${chain}"* ]]; then
        echo ""
        echo "--- post-upgrade-pallet-test.ts ---"
        ((TOTAL++)) || true
        if bun run "${SCRIPT_DIR}/post-upgrade-pallet-test.ts" --port "${port}" 2>&1; then
            ((PASSED++)) || true
        else
            ((FAILED++)) || true
        fi
    fi

    # Stop Chopsticks
    kill "${chopsticks_pid}" 2>/dev/null || true
    wait "${chopsticks_pid}" 2>/dev/null || true
    sleep 2
}

# Main
echo "Chopsticks Upgrade Test Orchestrator"
echo "====================================="

if [[ "${CHAIN}" == "all" ]]; then
    for c in leafchain-sand-testnet leafchain-avatect-mainnet leafchain-lmt-testnet leafchain-lmt-mainnet leafchain-ecq-testnet leafchain-ecq-mainnet rootchain-testnet rootchain-mainnet; do
        run_chain_test "${c}"
    done
else
    run_chain_test "${CHAIN}"
fi

echo ""
echo "═══════════════════════════════════════════════"
echo "  TOTAL: ${TOTAL} tests, ${PASSED} passed, ${FAILED} failed"
echo "═══════════════════════════════════════════════"

exit "${FAILED}"
