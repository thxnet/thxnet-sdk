#!/usr/bin/env bash
# Chopsticks upgrade test orchestrator
# Runs upgrade-test.ts and post-upgrade-pallet-test.ts against each chain config.
# Also supports XCM multi-chain mode via the 'xcm' subcommand.
#
# Usage:
#   ./scripts/chopsticks/chopsticks-test.sh [chain]
#   ./scripts/chopsticks/chopsticks-test.sh xcm
#
#   chain: leafchain-sand-testnet (default), leafchain-avatect-mainnet, leafchain-lmt-testnet,
#          leafchain-lmt-mainnet, leafchain-ecq-testnet, leafchain-ecq-mainnet,
#          rootchain-testnet, rootchain-mainnet, all
#   xcm:  XCM multi-chain test — forks rootchain-testnet (relay) + leafchain-sand-testnet (para)
#         and runs xcm-upgrade-test.ts against both endpoints simultaneously.
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

XCM_RELAY_PORT=8100
XCM_PARA_PORT=8102
XCM_STARTUP_WAIT=90
XCM_LOG_DIR="${CI:+/tmp/chopsticks-xcm-logs}"
XCM_LOG_DIR="${XCM_LOG_DIR:-/tmp/chopsticks-xcm-logs}"

cleanup() {
    pkill -f "chopsticks.*${SCRIPT_DIR}" 2>/dev/null || true
}
trap cleanup EXIT

run_xcm_test() {
    local relay_wasm="${ROOTCHAIN_TESTNET_WASM}"
    local para_wasm="${LEAFCHAIN_WASM}"
    local relay_config="${SCRIPT_DIR}/rootchain-testnet.yml"
    local para_config="${SCRIPT_DIR}/leafchain-sand-testnet.yml"
    local relay_endpoint="ws://localhost:${XCM_RELAY_PORT}"
    local para_endpoint="ws://localhost:${XCM_PARA_PORT}"

    echo ""
    echo "═══════════════════════════════════════════════"
    echo "  XCM Test: rootchain-testnet (relay) + leafchain-sand-testnet (para)"
    echo "  Relay port: ${XCM_RELAY_PORT}  Para port: ${XCM_PARA_PORT}"
    echo "═══════════════════════════════════════════════"

    # Check WASMs exist
    if [[ ! -f "${relay_wasm}" ]]; then
        echo "SKIP: Relay WASM not found: ${relay_wasm}"
        ((FAILED++)) || true
        ((TOTAL++)) || true
        return
    fi
    if [[ ! -f "${para_wasm}" ]]; then
        echo "SKIP: Para WASM not found: ${para_wasm}"
        ((FAILED++)) || true
        ((TOTAL++)) || true
        return
    fi

    # Chopsticks xcm mode has no --relay-wasm / --para-wasm CLI flags.
    # The only supported mechanism is the `wasm-override:` key in each chain's
    # config file. Generate temporary configs that extend the base configs with
    # the wasm-override entry so we do not mutate the checked-in YML files.
    local tmp_dir
    tmp_dir="$(mktemp -d)"
    local tmp_relay_config="${tmp_dir}/rootchain-testnet.yml"
    local tmp_para_config="${tmp_dir}/leafchain-sand-testnet.yml"

    # Copy base configs and append wasm-override line
    cp "${relay_config}" "${tmp_relay_config}"
    echo "wasm-override: ${relay_wasm}" >> "${tmp_relay_config}"
    cp "${para_config}" "${tmp_para_config}"
    echo "wasm-override: ${para_wasm}" >> "${tmp_para_config}"

    # Ensure temp dir is removed on function exit
    cleanup_xcm_tmp() { rm -rf "${tmp_dir}"; }
    trap 'cleanup_xcm_tmp; cleanup' EXIT

    # Start Chopsticks in xcm mode. Redirect output to a log file (not /dev/null)
    # so that on failure we have diagnostics about what Chopsticks was doing.
    mkdir -p "${XCM_LOG_DIR}"
    local chopsticks_log="${XCM_LOG_DIR}/chopsticks-xcm.log"
    echo "Starting Chopsticks in xcm mode (log: ${chopsticks_log})..."
    bunx @acala-network/chopsticks xcm \
        -r "${tmp_relay_config}" \
        -p "${tmp_para_config}" \
        >"${chopsticks_log}" 2>&1 &
    local chopsticks_pid=$!

    # Port readiness via bash builtin /dev/tcp — avoids depending on `nc` which
    # is not guaranteed to be installed on all runner images (nc: command not
    # found silently fails the health check, producing the same symptom as a
    # genuinely slow startup).
    port_ready() {
        (timeout 1 bash -c ">/dev/tcp/localhost/$1") 2>/dev/null
    }

    # Wait for both endpoints to become ready
    echo "Waiting for both endpoints (up to ${XCM_STARTUP_WAIT}s)..."
    local waited=0
    local relay_ready=0
    local para_ready=0
    while [[ ${waited} -lt ${XCM_STARTUP_WAIT} ]]; do
        if [[ ${relay_ready} -eq 0 ]] && port_ready "${XCM_RELAY_PORT}"; then
            relay_ready=1
            echo "  Relay endpoint ready (${waited}s)"
        fi
        if [[ ${para_ready} -eq 0 ]] && port_ready "${XCM_PARA_PORT}"; then
            para_ready=1
            echo "  Para endpoint ready (${waited}s)"
        fi
        if [[ ${relay_ready} -eq 1 && ${para_ready} -eq 1 ]]; then
            break
        fi
        sleep 1
        ((waited++)) || true
    done

    # On failure, tail the Chopsticks log so operators can see what went wrong
    if [[ ${relay_ready} -eq 0 || ${para_ready} -eq 0 ]]; then
        echo "--- Chopsticks xcm log (last 50 lines) ---"
        tail -n 50 "${chopsticks_log}" 2>/dev/null || echo "(log file unreadable)"
        echo "--- end log ---"
    fi

    if ! kill -0 "${chopsticks_pid}" 2>/dev/null; then
        echo "FAIL: Chopsticks xcm process died during startup"
        ((FAILED++)) || true
        ((TOTAL++)) || true
        rm -rf "${tmp_dir}"
        trap cleanup EXIT
        return
    fi

    if [[ ${relay_ready} -eq 0 || ${para_ready} -eq 0 ]]; then
        echo "FAIL: Endpoint(s) not ready after ${XCM_STARTUP_WAIT}s (relay=${relay_ready} para=${para_ready})"
        kill "${chopsticks_pid}" 2>/dev/null || true
        wait "${chopsticks_pid}" 2>/dev/null || true
        ((FAILED++)) || true
        ((TOTAL++)) || true
        rm -rf "${tmp_dir}"
        trap cleanup EXIT
        return
    fi

    # Port open != chain RPC ready. Chopsticks xcm mode opens the WebSocket
    # listener before the chain state is fully initialized; polkadot.js's
    # ApiPromise.create waits for state_getMetadata to respond, which can
    # time out (default 60s) if the chain isn't ready. Poll via a minimal
    # JSON-RPC system_chain call until both chains respond, then proceed.
    rpc_ready() {
        local port="$1"
        local response
        response=$(curl -sf --max-time 3 \
            -H "Content-Type: application/json" \
            -d '{"id":1,"jsonrpc":"2.0","method":"system_chain","params":[]}' \
            "http://localhost:${port}" 2>/dev/null || true)
        [[ "${response}" == *'"result"'* ]]
    }

    echo "Waiting for RPC state to be ready (up to 60s)..."
    local rpc_wait=0
    local relay_rpc_ready=0
    local para_rpc_ready=0
    while [[ ${rpc_wait} -lt 60 ]]; do
        if [[ ${relay_rpc_ready} -eq 0 ]] && rpc_ready "${XCM_RELAY_PORT}"; then
            relay_rpc_ready=1
            echo "  Relay RPC ready (${rpc_wait}s)"
        fi
        if [[ ${para_rpc_ready} -eq 0 ]] && rpc_ready "${XCM_PARA_PORT}"; then
            para_rpc_ready=1
            echo "  Para RPC ready (${rpc_wait}s)"
        fi
        if [[ ${relay_rpc_ready} -eq 1 && ${para_rpc_ready} -eq 1 ]]; then
            break
        fi
        sleep 2
        rpc_wait=$((rpc_wait + 2))
    done

    if [[ ${relay_rpc_ready} -eq 0 || ${para_rpc_ready} -eq 0 ]]; then
        echo "WARN: RPC(s) not responsive after 60s (relay=${relay_rpc_ready} para=${para_rpc_ready}) — proceeding anyway"
        echo "--- Chopsticks xcm log (last 50 lines) ---"
        tail -n 50 "${chopsticks_log}" 2>/dev/null || echo "(log file unreadable)"
        echo "--- end log ---"
    fi

    # Run xcm-upgrade-test.ts against both endpoints
    echo "--- xcm-upgrade-test.ts ---"
    ((TOTAL++)) || true
    if bun run "${SCRIPT_DIR}/xcm-upgrade-test.ts" \
        --relay-endpoint "${relay_endpoint}" \
        --para-endpoint "${para_endpoint}" 2>&1; then
        ((PASSED++)) || true
    else
        ((FAILED++)) || true
    fi

    # Stop Chopsticks
    kill "${chopsticks_pid}" 2>/dev/null || true
    wait "${chopsticks_pid}" 2>/dev/null || true
    sleep 2

    # Remove temp configs
    rm -rf "${tmp_dir}"
    # Restore simple cleanup trap (temp dir already gone)
    trap cleanup EXIT
}

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

run_fast_forward_test() {
    local chain="$1"
    local n_blocks="${2:-50}"
    local config="${CHAIN_CONFIG[$chain]}"
    local wasm="${CHAIN_WASM[$chain]}"
    local port="${CHAIN_PORT[$chain]}"

    echo ""
    echo "═══════════════════════════════════════════════"
    echo "  Fast-forward test: ${chain} (port ${port}, ${n_blocks} blocks)"
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

    # Run fast-forward-test.ts.
    # --try-state is NOT passed: the wasm-runtimes artifact consumed by this job
    # is built without --features try-runtime; passing --try-state would cause
    # every block to fail with "unknown function: TryRuntime_execute_block".
    echo "--- fast-forward-test.ts ---"
    ((TOTAL++)) || true
    if bun run "${SCRIPT_DIR}/fast-forward-test.ts" \
        --port "${port}" \
        --n-blocks "${n_blocks}" \
        --chain "${chain}" \
        2>&1; then
        ((PASSED++)) || true
    else
        ((FAILED++)) || true
    fi

    # Stop Chopsticks
    kill "${chopsticks_pid}" 2>/dev/null || true
    wait "${chopsticks_pid}" 2>/dev/null || true
    sleep 2
}

# Main
echo "Chopsticks Upgrade Test Orchestrator"
echo "====================================="

if [[ "${CHAIN}" == "xcm" || "${CHAIN}" == "xcm-rootchain-testnet-leafchain-sand-testnet" ]]; then
    run_xcm_test
elif [[ "${CHAIN}" == fast-forward-* ]]; then
    target_chain="${CHAIN#fast-forward-}"
    run_fast_forward_test "${target_chain}" "${N_BLOCKS:-50}"
elif [[ "${CHAIN}" == "all" ]]; then
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
