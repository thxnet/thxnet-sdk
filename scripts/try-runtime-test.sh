#!/usr/bin/env bash
# =============================================================================
# try-runtime migration verification for THXNet v1.12.0 upgrade
# =============================================================================
#
# Validates the entire migration chain against live chain state by:
# 1. Building runtime WASMs with the try-runtime feature
# 2. Running on-runtime-upgrade against live state (fetched via RPC)
# 3. Executing pre_upgrade() / post_upgrade() checks for each migration
# 4. Validating StorageVersion consistency across all pallets
#
# Prerequisites:
#   - Rust toolchain (same as CI: stable, tested with 1.77+)
#   - try-runtime CLI:
#       cargo install frame-try-runtime-cli --locked
#     OR build from the substrate tree:
#       cargo install --path substrate/utils/frame/try-runtime-cli --locked
#   - Network access to archive node endpoints (or local port-forwards)
#
# Usage:
#   ./scripts/try-runtime-test.sh [COMMAND]
#
# Commands:
#   build-rootchain     Build rootchain runtimes (mainnet + testnet) with try-runtime
#   build-leafchain     Build leafchain (general-runtime) with try-runtime
#   build-all           Build all runtimes with try-runtime
#   test-rootchain-testnet   Run migrations against rootchain testnet
#   test-rootchain-mainnet   Run migrations against rootchain mainnet
#   test-leafchain-sand      Run migrations against leafchain Sand (testnet)
#   test-leafchain-avatect   Run migrations against leafchain Avatect (mainnet)
#   test-all-testnet         Run all testnet chains (rootchain + leafchain)
#   test-all                 Run all chains (testnet first, then mainnet)
#   check                    Verify try-runtime CLI is installed and runtimes compile
#   help                     Show this help
#
# Environment variables:
#   CARGO_TARGET_DIR     Override cargo target directory (default: ./target)
#   TRY_RUNTIME_BIN      Path to try-runtime binary (default: try-runtime in PATH)
#   ROOTCHAIN_TESTNET_URI  Override testnet rootchain endpoint
#   ROOTCHAIN_MAINNET_URI  Override mainnet rootchain endpoint
#   LEAFCHAIN_SAND_URI     Override Sand leafchain endpoint
#   LEAFCHAIN_AVATECT_URI  Override Avatect leafchain endpoint
#   BLOCKTIME_ROOT         Rootchain block time in ms (default: 6000)
#   BLOCKTIME_LEAF         Leafchain block time in ms (default: 12000)
#   SKIP_BUILD             Set to 1 to skip WASM build (use pre-built artifacts)
#
# =============================================================================

set -euo pipefail

# ─── Configuration ───────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Runtime package names
ROOTCHAIN_MAINNET_PKG="thxnet-runtime"
ROOTCHAIN_TESTNET_PKG="thxnet-testnet-runtime"
LEAFCHAIN_PKG="general-runtime"

# WASM output paths (after cargo build --release)
WASM_DIR="${CARGO_TARGET_DIR:-${PROJECT_ROOT}/target}/release/wbuild"
ROOTCHAIN_MAINNET_WASM="${WASM_DIR}/${ROOTCHAIN_MAINNET_PKG}/thxnet_runtime.compact.compressed.wasm"
ROOTCHAIN_TESTNET_WASM="${WASM_DIR}/${ROOTCHAIN_TESTNET_PKG}/thxnet_testnet_runtime.compact.compressed.wasm"
LEAFCHAIN_WASM="${WASM_DIR}/${LEAFCHAIN_PKG}/general_runtime.compact.compressed.wasm"

# try-runtime CLI binary
TRY_RUNTIME_BIN="${TRY_RUNTIME_BIN:-try-runtime}"

# Live chain RPC endpoints
ROOTCHAIN_TESTNET_URI="${ROOTCHAIN_TESTNET_URI:-wss://node.testnet.thxnet.org/archive-001/ws}"
ROOTCHAIN_MAINNET_URI="${ROOTCHAIN_MAINNET_URI:-wss://node.mainnet.thxnet.org/archive-001/ws}"
LEAFCHAIN_SAND_URI="${LEAFCHAIN_SAND_URI:-wss://node.sand.testnet.thxnet.org/archive-001/ws}"
LEAFCHAIN_AVATECT_URI="${LEAFCHAIN_AVATECT_URI:-wss://node.avatect.mainnet.thxnet.org/archive-001/ws}"

# Block times (ms)
BLOCKTIME_ROOT="${BLOCKTIME_ROOT:-6000}"
BLOCKTIME_LEAF="${BLOCKTIME_LEAF:-12000}"

# Skip build flag
SKIP_BUILD="${SKIP_BUILD:-0}"

# ─── Colors ──────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# ─── Helpers ─────────────────────────────────────────────────────────────────

log_info()    { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[OK]${NC}   $*"; }
log_warn()    { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error()   { echo -e "${RED}[FAIL]${NC} $*"; }
log_step()    { echo -e "\n${CYAN}════════════════════════════════════════════════════════════${NC}"; \
                echo -e "${CYAN}  $*${NC}"; \
                echo -e "${CYAN}════════════════════════════════════════════════════════════${NC}\n"; }

check_try_runtime_cli() {
    if ! command -v "${TRY_RUNTIME_BIN}" &>/dev/null; then
        log_error "try-runtime CLI not found at '${TRY_RUNTIME_BIN}'"
        echo ""
        echo "Install it with one of:"
        echo "  cargo install frame-try-runtime-cli --locked"
        echo "  cargo install --path substrate/utils/frame/try-runtime-cli --locked"
        echo ""
        echo "Or set TRY_RUNTIME_BIN=/path/to/try-runtime"
        return 1
    fi
    local version
    version="$("${TRY_RUNTIME_BIN}" --version 2>/dev/null || echo 'unknown')"
    log_success "try-runtime CLI found: ${version}"
}

check_wasm_exists() {
    local wasm_path="$1"
    local name="$2"
    if [[ ! -f "${wasm_path}" ]]; then
        log_error "WASM not found: ${wasm_path}"
        echo "  Build it first with: $0 build-${name}"
        return 1
    fi
    local size
    size=$(du -h "${wasm_path}" | cut -f1)
    log_success "WASM found: ${name} (${size})"
}

# ─── Build Commands ──────────────────────────────────────────────────────────

build_rootchain() {
    if [[ "${SKIP_BUILD}" == "1" ]]; then
        log_warn "SKIP_BUILD=1, skipping WASM build"
        return 0
    fi

    log_step "Building rootchain runtimes with try-runtime feature"

    cd "${PROJECT_ROOT}"

    log_info "Building ${ROOTCHAIN_TESTNET_PKG}..."
    WASM_BUILD_WORKSPACE_HINT="${PROJECT_ROOT}" \
        cargo build --release --locked -p "${ROOTCHAIN_TESTNET_PKG}" --features try-runtime
    check_wasm_exists "${ROOTCHAIN_TESTNET_WASM}" "rootchain-testnet"

    log_info "Building ${ROOTCHAIN_MAINNET_PKG}..."
    WASM_BUILD_WORKSPACE_HINT="${PROJECT_ROOT}" \
        cargo build --release --locked -p "${ROOTCHAIN_MAINNET_PKG}" --features try-runtime
    check_wasm_exists "${ROOTCHAIN_MAINNET_WASM}" "rootchain-mainnet"

    log_success "Rootchain runtimes built successfully"
}

build_leafchain() {
    if [[ "${SKIP_BUILD}" == "1" ]]; then
        log_warn "SKIP_BUILD=1, skipping WASM build"
        return 0
    fi

    log_step "Building leafchain runtime with try-runtime feature"

    cd "${PROJECT_ROOT}"

    log_info "Building ${LEAFCHAIN_PKG}..."
    WASM_BUILD_WORKSPACE_HINT="${PROJECT_ROOT}" \
        cargo build --release --locked -p "${LEAFCHAIN_PKG}" --features try-runtime
    check_wasm_exists "${LEAFCHAIN_WASM}" "leafchain"

    log_success "Leafchain runtime built successfully"
}

build_all() {
    build_rootchain
    build_leafchain
}

# ─── Test Commands ───────────────────────────────────────────────────────────

run_try_runtime() {
    local name="$1"
    local wasm_path="$2"
    local uri="$3"
    local blocktime="$4"

    log_step "try-runtime: ${name}"
    log_info "WASM:      ${wasm_path}"
    log_info "Endpoint:  ${uri}"
    log_info "Blocktime: ${blocktime}ms"
    echo ""

    check_wasm_exists "${wasm_path}" "${name}" || return 1

    export RUST_LOG="${RUST_LOG:-remote-ext=debug,runtime=debug}"

    # --disable-spec-version-check: required because we're jumping multiple
    #   spec versions (e.g., 94_000_001 -> 112_000_001) and the CLI would
    #   refuse the upgrade otherwise.
    #
    # --disable-mbm-checks: multi-block migration checks are not applicable
    #   since we use single-block OnRuntimeUpgrade migrations exclusively.
    #
    # --blocktime: used to calculate weight-to-time mapping for migration
    #   weight limit checks.
    #
    # --checks=all: run pre_upgrade() and post_upgrade() hooks for every
    #   migration — matches upstream polkadot-sdk CI behavior.

    local start_time exit_code
    start_time=$(date +%s)

    "${TRY_RUNTIME_BIN}" \
        --runtime "${wasm_path}" \
        on-runtime-upgrade \
        --blocktime "${blocktime}" \
        --disable-mbm-checks \
        --disable-spec-version-check \
        --checks=all \
        live --uri "${uri}" \
        && exit_code=0 || exit_code=$?

    local elapsed=$(( $(date +%s) - start_time ))

    echo ""
    if [[ ${exit_code} -eq 0 ]]; then
        log_success "${name}: ALL MIGRATIONS PASSED (${elapsed}s)"
    else
        log_error "${name}: MIGRATION FAILED (exit code ${exit_code}, ${elapsed}s)"
        return ${exit_code}
    fi
}

test_rootchain_testnet() {
    run_try_runtime \
        "rootchain-testnet" \
        "${ROOTCHAIN_TESTNET_WASM}" \
        "${ROOTCHAIN_TESTNET_URI}" \
        "${BLOCKTIME_ROOT}"
}

test_rootchain_mainnet() {
    run_try_runtime \
        "rootchain-mainnet" \
        "${ROOTCHAIN_MAINNET_WASM}" \
        "${ROOTCHAIN_MAINNET_URI}" \
        "${BLOCKTIME_ROOT}"
}

test_leafchain_sand() {
    run_try_runtime \
        "leafchain-sand" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_SAND_URI}" \
        "${BLOCKTIME_LEAF}"
}

test_leafchain_avatect() {
    run_try_runtime \
        "leafchain-avatect" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_AVATECT_URI}" \
        "${BLOCKTIME_LEAF}"
}

test_all_testnet() {
    log_step "Running try-runtime against all TESTNET chains"

    local failed=0

    test_rootchain_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Rootchain testnet failed — aborting remaining tests"
        return 1
    fi

    test_leafchain_sand || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Sand failed"
        return 1
    fi

    log_success "All testnet chains passed"
}

test_all() {
    log_step "Running try-runtime against ALL chains (testnet first, then mainnet)"

    local failed=0

    # Testnet first (lower risk)
    test_rootchain_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Rootchain testnet failed — aborting"
        return 1
    fi

    test_leafchain_sand || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Sand (testnet) failed — aborting"
        return 1
    fi

    log_success "Testnet chains passed, proceeding to mainnet..."
    echo ""

    # Mainnet (higher risk, run after testnet passes)
    test_rootchain_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Rootchain mainnet failed — aborting"
        return 1
    fi

    test_leafchain_avatect || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Avatect (mainnet) failed"
        return 1
    fi

    echo ""
    log_step "ALL CHAINS PASSED"
    print_verification_checklist
}

# ─── Check Command ───────────────────────────────────────────────────────────

do_check() {
    log_step "Pre-flight checks"

    local ok=0

    # 1. try-runtime CLI
    check_try_runtime_cli || ok=1

    # 2. Rust toolchain
    if command -v rustc &>/dev/null; then
        log_success "rustc: $(rustc --version)"
    else
        log_error "rustc not found"
        ok=1
    fi

    # 3. Check if WASMs exist (informational, not fatal)
    echo ""
    log_info "Checking for pre-built WASMs..."
    check_wasm_exists "${ROOTCHAIN_TESTNET_WASM}" "rootchain-testnet" 2>/dev/null || \
        log_warn "Rootchain testnet WASM not built yet"
    check_wasm_exists "${ROOTCHAIN_MAINNET_WASM}" "rootchain-mainnet" 2>/dev/null || \
        log_warn "Rootchain mainnet WASM not built yet"
    check_wasm_exists "${LEAFCHAIN_WASM}" "leafchain" 2>/dev/null || \
        log_warn "Leafchain WASM not built yet"

    # 4. Quick cargo check (compile test without full build)
    echo ""
    log_info "Running cargo check with try-runtime feature (rootchain testnet)..."
    cd "${PROJECT_ROOT}"
    if WASM_BUILD_WORKSPACE_HINT="${PROJECT_ROOT}" \
        cargo check -p "${ROOTCHAIN_TESTNET_PKG}" --features try-runtime 2>&1; then
        log_success "Rootchain testnet compiles with try-runtime"
    else
        log_error "Rootchain testnet failed to compile with try-runtime"
        ok=1
    fi

    log_info "Running cargo check with try-runtime feature (general-runtime)..."
    if WASM_BUILD_WORKSPACE_HINT="${PROJECT_ROOT}" \
        cargo check -p "${LEAFCHAIN_PKG}" --features try-runtime 2>&1; then
        log_success "Leafchain compiles with try-runtime"
    else
        log_error "Leafchain failed to compile with try-runtime"
        ok=1
    fi

    echo ""
    if [[ ${ok} -eq 0 ]]; then
        log_success "All pre-flight checks passed"
    else
        log_error "Some checks failed — see above"
    fi
    return ${ok}
}

# ─── Verification Checklist ──────────────────────────────────────────────────

print_verification_checklist() {
    echo ""
    echo "╔══════════════════════════════════════════════════════════════════╗"
    echo "║           POST-try-runtime VERIFICATION CHECKLIST              ║"
    echo "╠══════════════════════════════════════════════════════════════════╣"
    echo "║                                                                ║"
    echo "║  1. StorageVersion consistency                                 ║"
    echo "║     [ ] All on-chain versions == in-code versions              ║"
    echo "║     [ ] No 'version mismatch' errors in output                 ║"
    echo "║                                                                ║"
    echo "║  2. Migration execution                                        ║"
    echo "║     [ ] No panics or runtime errors                            ║"
    echo "║     [ ] No 'migration skipped' warnings for critical items:    ║"
    echo "║         - Configuration v4->v5->v6->v7->v8->v9->v10->v11->v12  ║"
    echo "║         - NominationPools v4->v5->v6->v7->v8                   ║"
    echo "║         - Staking v13->v14->v15                                ║"
    echo "║         - Session keys upgrade (ImOnline removal)              ║"
    echo "║         - Scheduler v0->v1->v2                                 ║"
    echo "║         - Identity v0->v1                                      ║"
    echo "║         - Grandpa v4->v5                                       ║"
    echo "║                                                                ║"
    echo "║  3. Weight consumption                                         ║"
    echo "║     [ ] Total migration weight < max_block weight              ║"
    echo "║     [ ] Check output for 'weight consumed' vs 'weight limit'   ║"
    echo "║                                                                ║"
    echo "║  4. pre_upgrade / post_upgrade checks                          ║"
    echo "║     [ ] All pre_upgrade() hooks returned Ok                    ║"
    echo "║     [ ] All post_upgrade() hooks returned Ok                   ║"
    echo "║     [ ] UpgradeSessionKeys: keys match before/after            ║"
    echo "║                                                                ║"
    echo "║  5. Leafchain-specific checks                                  ║"
    echo "║     [ ] DmpQueue StorageVersion stamped to v2                  ║"
    echo "║     [ ] XcmpQueue v1->v2->v3->v4 all passed                    ║"
    echo "║     [ ] CollatorSelection v0->v1->v2 passed                    ║"
    echo "║     [ ] Crowdfunding stamped/migrated to v3                    ║"
    echo "║     [ ] RWA stamped to v5                                      ║"
    echo "║                                                                ║"
    echo "║  6. Rootchain-specific checks                                  ║"
    echo "║     [ ] FixGrandpaFinalityDeadlock no-op on post-14.25M blocks ║"
    echo "║     [ ] ImOnline pallet fully removed                          ║"
    echo "║     [ ] Tips pallet fully removed                              ║"
    echo "║     [ ] Crowdloan TrackInactiveV2 applied                      ║"
    echo "║     [ ] Parachains Inclusion v0->v1 applied                    ║"
    echo "║                                                                ║"
    echo "║  7. Spec version                                               ║"
    echo "║     [ ] Output shows spec_version = 112_000_001 (rootchain)    ║"
    echo "║     [ ] Output shows spec_version = 16 (leafchain)             ║"
    echo "║                                                                ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    echo ""
}

# ─── Help ────────────────────────────────────────────────────────────────────

show_help() {
    echo "Usage:"
    echo "  ./scripts/try-runtime-test.sh [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  build-rootchain          Build rootchain runtimes (mainnet + testnet) with try-runtime"
    echo "  build-leafchain          Build leafchain (general-runtime) with try-runtime"
    echo "  build-all                Build all runtimes with try-runtime"
    echo "  test-rootchain-testnet   Run migrations against rootchain testnet"
    echo "  test-rootchain-mainnet   Run migrations against rootchain mainnet"
    echo "  test-leafchain-sand      Run migrations against leafchain Sand (testnet)"
    echo "  test-leafchain-avatect   Run migrations against leafchain Avatect (mainnet)"
    echo "  test-all-testnet         Run all testnet chains (rootchain + leafchain)"
    echo "  test-all                 Run all chains (testnet first, then mainnet)"
    echo "  check                    Verify try-runtime CLI is installed and runtimes compile"
    echo "  checklist                Print post-run verification checklist"
    echo "  help                     Show this help"
    echo ""
    echo "Environment variables:"
    echo "  CARGO_TARGET_DIR          Override cargo target directory"
    echo "  TRY_RUNTIME_BIN           Path to try-runtime binary"
    echo "  ROOTCHAIN_TESTNET_URI     Override testnet rootchain endpoint"
    echo "  ROOTCHAIN_MAINNET_URI     Override mainnet rootchain endpoint"
    echo "  LEAFCHAIN_SAND_URI        Override Sand leafchain endpoint"
    echo "  LEAFCHAIN_AVATECT_URI     Override Avatect leafchain endpoint"
    echo "  BLOCKTIME_ROOT            Rootchain block time in ms (default: 6000)"
    echo "  BLOCKTIME_LEAF            Leafchain block time in ms (default: 12000)"
    echo "  SKIP_BUILD                Set to 1 to skip WASM build"
    echo ""
    print_verification_checklist
}

# ─── Main ────────────────────────────────────────────────────────────────────

main() {
    local cmd="${1:-help}"

    cd "${PROJECT_ROOT}"

    case "${cmd}" in
        build-rootchain)      build_rootchain ;;
        build-leafchain)      build_leafchain ;;
        build-all)            build_all ;;
        test-rootchain-testnet)   test_rootchain_testnet ;;
        test-rootchain-mainnet)   test_rootchain_mainnet ;;
        test-leafchain-sand)      test_leafchain_sand ;;
        test-leafchain-avatect)   test_leafchain_avatect ;;
        test-all-testnet)         test_all_testnet ;;
        test-all)                 test_all ;;
        check)                    do_check ;;
        checklist)                print_verification_checklist ;;
        help|--help|-h)           show_help ;;
        *)
            log_error "Unknown command: ${cmd}"
            echo "Run '$0 help' for usage."
            exit 1
            ;;
    esac
}

main "$@"
