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
#   test-leafchain-sand-testnet      Run migrations against leafchain Sand (testnet)
#   test-leafchain-avatect-mainnet   Run migrations against leafchain Avatect (mainnet)
#   test-leafchain-lmt-testnet   Run migrations against leafchain LMT (testnet)
#   test-leafchain-lmt-mainnet   Run migrations against leafchain LMT (mainnet)
#   test-leafchain-ecq-testnet   Run migrations against leafchain ECQ (testnet)
#   test-leafchain-ecq-mainnet   Run migrations against leafchain ECQ (mainnet)
#   test-all-testnet         Run all testnet chains (rootchain + leafchain)
#   test-all                 Run all chains (testnet first, then mainnet)
#   test-idempotency-rootchain-testnet   Idempotency test: rootchain testnet
#   test-idempotency-rootchain-mainnet   Idempotency test: rootchain mainnet
#   test-idempotency-leafchain-sand-testnet      Idempotency test: leafchain Sand
#   test-idempotency-leafchain-avatect-mainnet       Idempotency test: leafchain Avatect
#   test-idempotency-leafchain-lmt-testnet  Idempotency test: leafchain LMT testnet
#   test-idempotency-leafchain-lmt-mainnet  Idempotency test: leafchain LMT mainnet
#   test-idempotency-leafchain-ecq-testnet  Idempotency test: leafchain ECQ testnet
#   test-idempotency-leafchain-ecq-mainnet  Idempotency test: leafchain ECQ mainnet
#   test-idempotency-all-testnet             Idempotency test: all testnet chains
#   test-idempotency-all                 Idempotency test: all chains
#   create-snapshot-rootchain-testnet    Save rootchain testnet state as snapshot
#   create-snapshot-rootchain-mainnet    Save rootchain mainnet state as snapshot
#   create-snapshot-leafchain-sand-testnet       Save leafchain Sand state as snapshot
#   create-snapshot-leafchain-avatect-mainnet        Save leafchain Avatect state as snapshot
#   create-snapshot-leafchain-lmt-testnet   Save leafchain LMT testnet state as snapshot
#   create-snapshot-leafchain-lmt-mainnet   Save leafchain LMT mainnet state as snapshot
#   create-snapshot-leafchain-ecq-testnet   Save leafchain ECQ testnet state as snapshot
#   create-snapshot-leafchain-ecq-mainnet   Save leafchain ECQ mainnet state as snapshot
#   create-snapshot-all-testnet              Save all testnet chain snapshots
#   create-snapshot-all                  Save all chain snapshots
#   list-snapshots           List all saved snapshots with name, size, date
#   clean-snapshots          Remove snapshots older than SNAPSHOT_MAX_AGE_HOURS
#   test-from-snapshot-rootchain-testnet [path]  Test rootchain testnet from snapshot
#   test-from-snapshot-rootchain-mainnet [path]  Test rootchain mainnet from snapshot
#   test-from-snapshot-leafchain-sand-testnet [path]     Test leafchain Sand from snapshot
#   test-from-snapshot-leafchain-avatect-mainnet [path]      Test leafchain Avatect from snapshot
#   test-from-snapshot-leafchain-lmt-testnet [path] Test leafchain LMT testnet from snapshot
#   test-from-snapshot-leafchain-lmt-mainnet [path] Test leafchain LMT mainnet from snapshot
#   test-from-snapshot-leafchain-ecq-testnet [path] Test leafchain ECQ testnet from snapshot
#   test-from-snapshot-leafchain-ecq-mainnet [path] Test leafchain ECQ mainnet from snapshot
#   test-from-snapshot-all-testnet                   Test all testnet chains from snapshots
#   test-from-snapshot-all                       Test all chains from snapshots
#   test-pallet <pallet> <chain>     Test single pallet migration on chain
#   test-pallet-critical <chain>     Test all critical pallets on chain
#   test-pallet-matrix [chains...] [-- pallets...]
#                                    Matrix test: pallets x chains grid
#                                    Also reads MATRIX_CHAINS / MATRIX_PALLETS env vars
#   verify-ci-readiness      Comprehensive pre-flight check for CI pipeline
#   check                    Verify try-runtime CLI is installed and runtimes compile
#   version                  Print script version and capabilities
#   help                     Show this help
#
# Environment variables:
#   CARGO_TARGET_DIR     Override cargo target directory (default: ./target)
#   TRY_RUNTIME_BIN      Path to try-runtime binary (default: try-runtime in PATH)
#   ROOTCHAIN_TESTNET_URI  Override testnet rootchain endpoint
#   ROOTCHAIN_MAINNET_URI  Override mainnet rootchain endpoint
#   LEAFCHAIN_SAND_TESTNET_URI     Override Sand leafchain endpoint
#   LEAFCHAIN_AVATECT_MAINNET_URI  Override Avatect leafchain endpoint
#   LEAFCHAIN_LMT_TESTNET_URI     Override LMT testnet leafchain endpoint
#   LEAFCHAIN_LMT_MAINNET_URI     Override LMT mainnet leafchain endpoint
#   LEAFCHAIN_ECQ_TESTNET_URI     Override ECQ testnet leafchain endpoint
#   LEAFCHAIN_ECQ_MAINNET_URI     Override ECQ mainnet leafchain endpoint
#   BLOCKTIME_ROOT         Rootchain block time in ms (default: 6000)
#   BLOCKTIME_LEAF         Leafchain block time in ms (default: 12000)
#   SKIP_BUILD             Set to 1 to skip WASM build (use pre-built artifacts)
#   DRY_RUN                Set to 1 to print try-runtime commands without executing
#   SNAPSHOT_DIR           Directory for snapshot files (default: target/try-runtime-snapshots)
#   KEEP_SNAPSHOTS         Set to 1 to preserve snapshot files after tests
#   SNAPSHOT_AT_BLOCK      Pin snapshot to a specific block hash (reproducible)
#   SNAPSHOT_MAX_AGE_HOURS Max age (hours) for clean-snapshots (default: 24)
#   SNAPSHOT_MAX_SIZE_GB  Max total snapshot dir size in GB; force-deletes all if exceeded (default: 50)
#   MATRIX_CHAINS          Space-separated chain list for test-pallet-matrix
#   MATRIX_PALLETS         Space-separated pallet list for test-pallet-matrix (default: KNOWN_PALLETS)
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
LEAFCHAIN_SAND_TESTNET_URI="${LEAFCHAIN_SAND_TESTNET_URI:-wss://node.sand.testnet.thxnet.org/archive-001/ws}"
LEAFCHAIN_AVATECT_MAINNET_URI="${LEAFCHAIN_AVATECT_MAINNET_URI:-wss://node.avatect.mainnet.thxnet.org/archive-001/ws}"
LEAFCHAIN_LMT_TESTNET_URI="${LEAFCHAIN_LMT_TESTNET_URI:-wss://node.lmt.testnet.thxnet.org/archive-001/ws}"
LEAFCHAIN_LMT_MAINNET_URI="${LEAFCHAIN_LMT_MAINNET_URI:-wss://node.lmt.mainnet.thxnet.org/archive-001/ws}"
LEAFCHAIN_ECQ_TESTNET_URI="${LEAFCHAIN_ECQ_TESTNET_URI:-wss://node.ecq.testnet.thxnet.org/archive-001/ws}"
LEAFCHAIN_ECQ_MAINNET_URI="${LEAFCHAIN_ECQ_MAINNET_URI:-wss://node.ecq.mainnet.thxnet.org/archive-001/ws}"

# Block times (ms)
BLOCKTIME_ROOT="${BLOCKTIME_ROOT:-6000}"
BLOCKTIME_LEAF="${BLOCKTIME_LEAF:-12000}"

# Skip build flag
SKIP_BUILD="${SKIP_BUILD:-0}"

# Dry run mode: print try-runtime commands but don't execute them
DRY_RUN="${DRY_RUN:-0}"

# Snapshot directory for idempotency testing
SNAPSHOT_DIR="${SNAPSHOT_DIR:-${PROJECT_ROOT}/target/try-runtime-snapshots}"

# Keep snapshots after idempotency test (default: clean up)
KEEP_SNAPSHOTS="${KEEP_SNAPSHOTS:-0}"

# Optional block hash for reproducible snapshots (unset = latest finalized)
SNAPSHOT_AT_BLOCK="${SNAPSHOT_AT_BLOCK:-}"

# Maximum age (hours) for snapshot cleanup (default: 24)
SNAPSHOT_MAX_AGE_HOURS="${SNAPSHOT_MAX_AGE_HOURS:-24}"

# Maximum total size (GB) for snapshot directory (default: 50)
# After age-based cleanup, if the directory still exceeds this size,
# ALL snapshots are force-deleted as a safety valve against disk fill.
SNAPSHOT_MAX_SIZE_GB="${SNAPSHOT_MAX_SIZE_GB:-50}"

# Critical pallets to test with test-pallet-critical
# These are the pallets with known migration gaps in v1.12.0.
KNOWN_PALLETS=(
    nomination_pools
    staking
    session
    crowdfunding
    rwa
    dao
    grandpa
    scheduler
    identity
    xcmp_queue
    collator_selection
)

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

# get_dir_size_bytes <dir>
#   Portable directory size in bytes (approximate).
#   Uses `du -sk` (POSIX) instead of GNU-only `du -sb` so this works on macOS.
#   Precision is 1 KiB — sufficient for the size-guard threshold comparisons.
get_dir_size_bytes() {
    local dir="$1"
    local size_kb
    size_kb=$(du -sk "${dir}" 2>/dev/null | cut -f1 || echo 0)
    echo $(( size_kb * 1024 ))
}

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

# snapshot_supported
#   Detects whether the installed try-runtime CLI supports --create-snapshot.
#   try-runtime v0.10.1 does NOT have this flag; future versions may add it.
#   Checks the `on-runtime-upgrade live` subcommand help where the flag lives.
#   Returns 0 (true) if supported, 1 (false) otherwise.
snapshot_supported() {
    # Guard: if the binary isn't even available, snapshots are not supported
    command -v "${TRY_RUNTIME_BIN}" &>/dev/null || return 1
    # --help may exit non-zero on some CLIs, so tolerate that
    "${TRY_RUNTIME_BIN}" on-runtime-upgrade live --help 2>&1 \
        | grep -q -- '--create-snapshot'
}

# ── Snapshot capability detection ───────────────────────────────────────────
# Detected once at script load time. Exported so CI (Turn 4) can consume it.
# When try-runtime is upgraded to support --create-snapshot, this flips
# automatically — no code changes needed.
if snapshot_supported; then
    SNAPSHOT_SUPPORTED=true
else
    SNAPSHOT_SUPPORTED=false
fi
export SNAPSHOT_SUPPORTED

# Common try-runtime on-runtime-upgrade flags used by every invocation:
#   --disable-mbm-checks:        multi-block migration checks are not applicable
#                                 (we use single-block OnRuntimeUpgrade exclusively)
#   --disable-spec-version-check: required because we jump multiple spec versions
#                                 (e.g., 94_000_001 -> 112_000_001)
#   --checks=all:                run pre_upgrade() and post_upgrade() hooks for
#                                every migration (matches upstream polkadot-sdk CI)
readonly COMMON_MIGRATION_FLAGS=(
    --disable-mbm-checks
    --disable-spec-version-check
    --checks=all
)

# echo_migration_flags
#   Prints COMMON_MIGRATION_FLAGS in the dry-run echo format (indented, backslash-
#   continued). Used by DRY_RUN blocks so echo output stays in sync with the
#   actual flags passed to try-runtime.
echo_migration_flags() {
    printf '    %s \\\n' "${COMMON_MIGRATION_FLAGS[@]}"
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

# resolve_chain_params <chain-name>
#   Maps a human-friendly chain name to (wasm_path, uri, blocktime).
#   Sets caller-visible variables: _wasm_path, _uri, _blocktime.
#   Returns 1 if the chain name is unrecognized.
resolve_chain_params() {
    local chain="$1"
    case "${chain}" in
        rootchain-testnet)
            _wasm_path="${ROOTCHAIN_TESTNET_WASM}"
            _uri="${ROOTCHAIN_TESTNET_URI}"
            _blocktime="${BLOCKTIME_ROOT}"
            ;;
        rootchain-mainnet)
            _wasm_path="${ROOTCHAIN_MAINNET_WASM}"
            _uri="${ROOTCHAIN_MAINNET_URI}"
            _blocktime="${BLOCKTIME_ROOT}"
            ;;
        leafchain-sand-testnet)
            _wasm_path="${LEAFCHAIN_WASM}"
            _uri="${LEAFCHAIN_SAND_TESTNET_URI}"
            _blocktime="${BLOCKTIME_LEAF}"
            ;;
        leafchain-avatect-mainnet)
            _wasm_path="${LEAFCHAIN_WASM}"
            _uri="${LEAFCHAIN_AVATECT_MAINNET_URI}"
            _blocktime="${BLOCKTIME_LEAF}"
            ;;
        leafchain-lmt-testnet)
            _wasm_path="${LEAFCHAIN_WASM}"
            _uri="${LEAFCHAIN_LMT_TESTNET_URI}"
            _blocktime="${BLOCKTIME_LEAF}"
            ;;
        leafchain-lmt-mainnet)
            _wasm_path="${LEAFCHAIN_WASM}"
            _uri="${LEAFCHAIN_LMT_MAINNET_URI}"
            _blocktime="${BLOCKTIME_LEAF}"
            ;;
        leafchain-ecq-testnet)
            _wasm_path="${LEAFCHAIN_WASM}"
            _uri="${LEAFCHAIN_ECQ_TESTNET_URI}"
            _blocktime="${BLOCKTIME_LEAF}"
            ;;
        leafchain-ecq-mainnet)
            _wasm_path="${LEAFCHAIN_WASM}"
            _uri="${LEAFCHAIN_ECQ_MAINNET_URI}"
            _blocktime="${BLOCKTIME_LEAF}"
            ;;
        *)
            log_error "Unknown chain: ${chain}"
            echo "  Valid chains: rootchain-testnet, rootchain-mainnet, leafchain-sand-testnet, leafchain-avatect-mainnet, leafchain-lmt-testnet, leafchain-lmt-mainnet, leafchain-ecq-testnet, leafchain-ecq-mainnet"
            return 1
            ;;
    esac
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

    # Common migration flags: see COMMON_MIGRATION_FLAGS definition at top.
    # --blocktime: used to calculate weight-to-time mapping for migration
    #   weight limit checks.

    local start_time exit_code
    start_time=$(date +%s)

    if [[ "${DRY_RUN}" == "1" ]]; then
        log_warn "[DRY RUN] Would execute:"
        echo "  ${TRY_RUNTIME_BIN} \\"
        echo "    --runtime ${wasm_path} \\"
        echo "    on-runtime-upgrade \\"
        echo "    --blocktime ${blocktime} \\"
        echo_migration_flags
        echo "    live --uri ${uri}"
        exit_code=0
    else
        "${TRY_RUNTIME_BIN}" \
            --runtime "${wasm_path}" \
            on-runtime-upgrade \
            --blocktime "${blocktime}" \
            "${COMMON_MIGRATION_FLAGS[@]}" \
            live --uri "${uri}" \
            && exit_code=0 || exit_code=$?
    fi

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

test_leafchain_sand_testnet() {
    run_try_runtime \
        "leafchain-sand-testnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_SAND_TESTNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

test_leafchain_avatect_mainnet() {
    run_try_runtime \
        "leafchain-avatect-mainnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_AVATECT_MAINNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

test_leafchain_lmt_testnet() {
    run_try_runtime \
        "leafchain-lmt-testnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_LMT_TESTNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

test_leafchain_lmt_mainnet() {
    run_try_runtime \
        "leafchain-lmt-mainnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_LMT_MAINNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

test_leafchain_ecq_testnet() {
    run_try_runtime \
        "leafchain-ecq-testnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_ECQ_TESTNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

test_leafchain_ecq_mainnet() {
    run_try_runtime \
        "leafchain-ecq-mainnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_ECQ_MAINNET_URI}" \
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

    test_leafchain_sand_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Sand failed"
        return 1
    fi

    test_leafchain_lmt_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain LMT testnet failed"
        return 1
    fi

    test_leafchain_ecq_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain ECQ testnet failed"
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

    test_leafchain_sand_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Sand (testnet) failed — aborting"
        return 1
    fi

    test_leafchain_lmt_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain LMT testnet failed — aborting"
        return 1
    fi

    test_leafchain_ecq_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain ECQ testnet failed — aborting"
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

    test_leafchain_avatect_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Avatect (mainnet) failed"
        return 1
    fi

    test_leafchain_lmt_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain LMT mainnet failed"
        return 1
    fi

    test_leafchain_ecq_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain ECQ mainnet failed"
        return 1
    fi

    echo ""
    log_step "ALL CHAINS PASSED"
    print_verification_checklist
}

# ─── Idempotency Testing ─────────────────────────────────────────────────────
#
# Verifies that migrations are deterministic and safe to re-run by:
#   1. Creating a snapshot of the pre-migration state from the live chain
#   2. Running on-runtime-upgrade from that snapshot (first pass)
#   3. Running on-runtime-upgrade from the same snapshot again (second pass)
#   4. Asserting both passes succeed and produce identical key metrics
#
# The snapshot file captures pre-migration state, so both passes start from
# the exact same input — any difference in output indicates non-determinism.

# extract_key_metrics <logfile>
#   Extracts deterministic, comparable lines from try-runtime output.
#   Filters OUT timestamps and timing info (non-deterministic by nature).
#   Keeps: weight consumed, migration results, version info, check results.
extract_key_metrics() {
    local logfile="$1"
    # Keep lines that contain key migration output indicators.
    # Exclude lines that are purely timing/timestamp (non-deterministic).
    # NOTE: Do NOT exclude bare 'time:' — it matches 'runtime:' as a substring,
    #       which would filter out virtually all meaningful try-runtime output.
    #       'timestamp' already covers timestamp-style lines.
    grep -iE '(weight|migration|version|check|upgrade|storage|pallet|executed|consumed|limit|spec)' \
        "${logfile}" \
        | grep -vE '(timestamp|elapsed|seconds|date)' \
        | sort \
        || true
}

test_idempotency() {
    local name="$1"
    local wasm_path="$2"
    local uri="$3"
    local blocktime="$4"

    log_step "Idempotency test: ${name}"

    # ── Capability gate ─────────────────────────────────────────────────
    # Idempotency testing requires --create-snapshot to capture pre-migration
    # state (Step 1). Without it, the entire test is non-functional.
    # When try-runtime is upgraded, SNAPSHOT_SUPPORTED will flip to true
    # automatically and this code path will activate without changes.
    if [[ "${SNAPSHOT_SUPPORTED}" != "true" ]]; then
        log_warn "Skipping idempotency test: --create-snapshot not supported by installed try-runtime"
        log_info "  SNAPSHOT_SUPPORTED=${SNAPSHOT_SUPPORTED}"
        log_info "  Upgrade try-runtime CLI to a version that supports --create-snapshot"
        return 0
    fi

    log_info "WASM:      ${wasm_path}"
    log_info "Endpoint:  ${uri}"
    log_info "Blocktime: ${blocktime}ms"
    echo ""

    check_wasm_exists "${wasm_path}" "${name}" || return 1

    # Ensure snapshot directory exists
    mkdir -p "${SNAPSHOT_DIR}"

    local snapshot_file="${SNAPSHOT_DIR}/${name}-idempotency-$(date +%Y%m%d-%H%M%S).snap"
    local log_pass1="${SNAPSHOT_DIR}/${name}-idempotency-pass1.log"
    local log_pass2="${SNAPSHOT_DIR}/${name}-idempotency-pass2.log"
    local metrics_pass1="${SNAPSHOT_DIR}/${name}-idempotency-metrics1.txt"
    local metrics_pass2="${SNAPSHOT_DIR}/${name}-idempotency-metrics2.txt"

    # Cleanup handler — remove temp files unless KEEP_SNAPSHOTS=1
    cleanup_idempotency() {
        # Remove the trap first to prevent re-entry
        trap - EXIT INT TERM
        if [[ "${KEEP_SNAPSHOTS}" != "1" ]]; then
            rm -f "${snapshot_file}" "${log_pass1}" "${log_pass2}" \
                  "${metrics_pass1}" "${metrics_pass2}"
            log_info "Cleaned up temporary files"
        else
            log_info "KEEP_SNAPSHOTS=1, preserving files:"
            log_info "  Snapshot: ${snapshot_file}"
            log_info "  Pass 1 log: ${log_pass1}"
            log_info "  Pass 2 log: ${log_pass2}"
        fi
    }

    # Register trap so that SIGINT/SIGTERM during long-running passes still
    # cleans up snapshot and log files (prevents disk waste from aborted runs).
    trap cleanup_idempotency EXIT INT TERM

    export RUST_LOG="${RUST_LOG:-remote-ext=debug,runtime=debug}"

    # ── Step 1: Create snapshot from live chain ──────────────────────────
    log_info "Step 1/4: Creating snapshot from live chain..."
    log_info "  Snapshot path: ${snapshot_file}"

    local exit_code_snapshot
    if [[ "${DRY_RUN}" == "1" ]]; then
        log_warn "[DRY RUN] Would execute:"
        echo "  ${TRY_RUNTIME_BIN} \\"
        echo "    --runtime ${wasm_path} \\"
        echo "    on-runtime-upgrade \\"
        echo "    --blocktime ${blocktime} \\"
        echo_migration_flags
        echo "    live --uri ${uri} --create-snapshot ${snapshot_file}"
        exit_code_snapshot=0
    else
        "${TRY_RUNTIME_BIN}" \
            --runtime "${wasm_path}" \
            on-runtime-upgrade \
            --blocktime "${blocktime}" \
            "${COMMON_MIGRATION_FLAGS[@]}" \
            live --uri "${uri}" --create-snapshot "${snapshot_file}" \
            && exit_code_snapshot=0 || exit_code_snapshot=$?
    fi

    if [[ ${exit_code_snapshot} -ne 0 ]]; then
        log_error "Snapshot creation + initial migration failed (exit code ${exit_code_snapshot})"
        cleanup_idempotency
        return ${exit_code_snapshot}
    fi

    # In dry-run mode, create a placeholder snapshot file so subsequent logic works
    if [[ "${DRY_RUN}" == "1" && ! -f "${snapshot_file}" ]]; then
        touch "${snapshot_file}"
    fi

    if [[ ! -f "${snapshot_file}" ]]; then
        log_error "Snapshot file was not created: ${snapshot_file}"
        cleanup_idempotency
        return 1
    fi

    local snap_size
    snap_size=$(du -h "${snapshot_file}" | cut -f1)
    log_success "Snapshot created: ${snap_size}"

    # ── Step 2: First pass from snapshot ─────────────────────────────────
    log_info "Step 2/4: Running migration from snapshot (pass 1)..."

    # Temporarily disable errexit so a failing pipeline doesn't abort the
    # script — we need to inspect PIPESTATUS to get the try-runtime exit code
    # (element 0) rather than tee's (element 1). Neither `|| true` nor
    # `&& ... || ...` work here: both reset PIPESTATUS before we can read it.
    local exit_code_pass1
    if [[ "${DRY_RUN}" == "1" ]]; then
        log_warn "[DRY RUN] Would execute:"
        echo "  ${TRY_RUNTIME_BIN} \\"
        echo "    --runtime ${wasm_path} \\"
        echo "    on-runtime-upgrade \\"
        echo "    --blocktime ${blocktime} \\"
        echo_migration_flags
        echo "    snap --path ${snapshot_file}"
        echo "  2>&1 | tee ${log_pass1}"
        touch "${log_pass1}"
        exit_code_pass1=0
    else
        set +e
        "${TRY_RUNTIME_BIN}" \
            --runtime "${wasm_path}" \
            on-runtime-upgrade \
            --blocktime "${blocktime}" \
            "${COMMON_MIGRATION_FLAGS[@]}" \
            snap --path "${snapshot_file}" \
            2>&1 | tee "${log_pass1}"
        exit_code_pass1=${PIPESTATUS[0]}
        set -e
    fi

    echo ""
    if [[ ${exit_code_pass1} -eq 0 ]]; then
        log_success "Pass 1: SUCCEEDED"
    else
        log_error "Pass 1: FAILED (exit code ${exit_code_pass1})"
        cleanup_idempotency
        return ${exit_code_pass1}
    fi

    # ── Step 3: Second pass from snapshot ────────────────────────────────
    log_info "Step 3/4: Running migration from snapshot (pass 2)..."

    local exit_code_pass2
    if [[ "${DRY_RUN}" == "1" ]]; then
        log_warn "[DRY RUN] Would execute:"
        echo "  ${TRY_RUNTIME_BIN} \\"
        echo "    --runtime ${wasm_path} \\"
        echo "    on-runtime-upgrade \\"
        echo "    --blocktime ${blocktime} \\"
        echo_migration_flags
        echo "    snap --path ${snapshot_file}"
        echo "  2>&1 | tee ${log_pass2}"
        touch "${log_pass2}"
        exit_code_pass2=0
    else
        set +e
        "${TRY_RUNTIME_BIN}" \
            --runtime "${wasm_path}" \
            on-runtime-upgrade \
            --blocktime "${blocktime}" \
            "${COMMON_MIGRATION_FLAGS[@]}" \
            snap --path "${snapshot_file}" \
            2>&1 | tee "${log_pass2}"
        exit_code_pass2=${PIPESTATUS[0]}
        set -e
    fi

    echo ""
    if [[ ${exit_code_pass2} -eq 0 ]]; then
        log_success "Pass 2: SUCCEEDED"
    else
        log_error "Pass 2: FAILED (exit code ${exit_code_pass2})"
        cleanup_idempotency
        return ${exit_code_pass2}
    fi

    # ── Step 4: Compare key metrics ──────────────────────────────────────
    log_info "Step 4/4: Comparing key output metrics between passes..."

    extract_key_metrics "${log_pass1}" > "${metrics_pass1}"
    extract_key_metrics "${log_pass2}" > "${metrics_pass2}"

    if diff -u "${metrics_pass1}" "${metrics_pass2}" > /dev/null 2>&1; then
        log_success "IDEMPOTENCY VERIFIED: Both passes produced identical key metrics"
        echo ""
        log_info "Key metrics from both passes:"
        if [[ -s "${metrics_pass1}" ]]; then
            while IFS= read -r line; do
                echo "    ${line}"
            done < "${metrics_pass1}"
        else
            log_warn "No key metric lines captured (migration may produce minimal output)"
        fi
    else
        log_error "IDEMPOTENCY FAILURE: Passes produced different key metrics"
        echo ""
        log_error "Diff (--- pass 1, +++ pass 2):"
        diff -u "${metrics_pass1}" "${metrics_pass2}" || true
        cleanup_idempotency
        return 1
    fi

    echo ""
    log_success "${name}: IDEMPOTENCY TEST PASSED"
    cleanup_idempotency
}

test_idempotency_rootchain_testnet() {
    test_idempotency \
        "rootchain-testnet" \
        "${ROOTCHAIN_TESTNET_WASM}" \
        "${ROOTCHAIN_TESTNET_URI}" \
        "${BLOCKTIME_ROOT}"
}

test_idempotency_rootchain_mainnet() {
    test_idempotency \
        "rootchain-mainnet" \
        "${ROOTCHAIN_MAINNET_WASM}" \
        "${ROOTCHAIN_MAINNET_URI}" \
        "${BLOCKTIME_ROOT}"
}

test_idempotency_leafchain_sand_testnet() {
    test_idempotency \
        "leafchain-sand-testnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_SAND_TESTNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

test_idempotency_leafchain_avatect_mainnet() {
    test_idempotency \
        "leafchain-avatect-mainnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_AVATECT_MAINNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

test_idempotency_leafchain_lmt_testnet() {
    test_idempotency \
        "leafchain-lmt-testnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_LMT_TESTNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

test_idempotency_leafchain_lmt_mainnet() {
    test_idempotency \
        "leafchain-lmt-mainnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_LMT_MAINNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

test_idempotency_leafchain_ecq_testnet() {
    test_idempotency \
        "leafchain-ecq-testnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_ECQ_TESTNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

test_idempotency_leafchain_ecq_mainnet() {
    test_idempotency \
        "leafchain-ecq-mainnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_ECQ_MAINNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

test_idempotency_all_testnet() {
    log_step "Running idempotency tests against all TESTNET chains"

    local failed=0

    test_idempotency_rootchain_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Rootchain testnet idempotency failed — aborting remaining tests"
        return 1
    fi

    test_idempotency_leafchain_sand_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Sand idempotency failed"
        return 1
    fi

    test_idempotency_leafchain_lmt_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain LMT testnet idempotency failed"
        return 1
    fi

    test_idempotency_leafchain_ecq_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain ECQ testnet idempotency failed"
        return 1
    fi

    log_success "All testnet idempotency tests passed"
}

test_idempotency_all() {
    log_step "Running idempotency tests against ALL chains (testnet first, then mainnet)"

    local failed=0

    # Testnet first (lower risk)
    test_idempotency_rootchain_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Rootchain testnet idempotency failed — aborting"
        return 1
    fi

    test_idempotency_leafchain_sand_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Sand idempotency failed — aborting"
        return 1
    fi

    test_idempotency_leafchain_lmt_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain LMT testnet idempotency failed — aborting"
        return 1
    fi

    test_idempotency_leafchain_ecq_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain ECQ testnet idempotency failed — aborting"
        return 1
    fi

    log_success "Testnet idempotency passed, proceeding to mainnet..."
    echo ""

    # Mainnet (higher risk, run after testnet passes)
    test_idempotency_rootchain_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Rootchain mainnet idempotency failed — aborting"
        return 1
    fi

    test_idempotency_leafchain_avatect_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Avatect idempotency failed"
        return 1
    fi

    test_idempotency_leafchain_lmt_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain LMT mainnet idempotency failed"
        return 1
    fi

    test_idempotency_leafchain_ecq_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain ECQ mainnet idempotency failed"
        return 1
    fi

    echo ""
    log_step "ALL IDEMPOTENCY TESTS PASSED"
}

# ─── Snapshot Management ─────────────────────────────────────────────────────
#
# Creates offline snapshots of live chain state for reproducible try-runtime
# testing. The --create-snapshot flag on `live` saves the PRE-migration state
# fetched from RPC to a local file. Subsequent runs can use `snap --path` to
# test against this frozen state without network access.
#
# NOTE: Snapshot creation also runs the full migration (--checks=all), because
# try-runtime v0.10.1 does not support --checks=none. The snapshot captures
# the pre-migration state regardless — the migration is a side effect.

create_snapshot() {
    local name="$1"
    local wasm_path="$2"
    local uri="$3"
    local blocktime="$4"
    local output_path="${5:-}"

    log_step "Create snapshot: ${name}"

    # ── Capability gate ─────────────────────────────────────────────────
    # --create-snapshot is not supported in try-runtime v0.10.1.
    # When try-runtime is upgraded, SNAPSHOT_SUPPORTED will flip to true
    # automatically and this code path will activate without changes.
    if [[ "${SNAPSHOT_SUPPORTED}" != "true" ]]; then
        log_warn "Skipping snapshot creation: --create-snapshot not supported by installed try-runtime"
        log_info "  SNAPSHOT_SUPPORTED=${SNAPSHOT_SUPPORTED}"
        log_info "  Upgrade try-runtime CLI to a version that supports --create-snapshot"
        return 0
    fi

    log_info "WASM:      ${wasm_path}"
    log_info "Endpoint:  ${uri}"
    log_info "Blocktime: ${blocktime}ms"

    check_wasm_exists "${wasm_path}" "${name}" || return 1

    # Ensure snapshot directory exists
    mkdir -p "${SNAPSHOT_DIR}"

    # Default output path: <SNAPSHOT_DIR>/<chain-name>-<ISO-date>.snap
    if [[ -z "${output_path}" ]]; then
        output_path="${SNAPSHOT_DIR}/${name}-$(date +%Y-%m-%d).snap"
    fi
    log_info "Output:    ${output_path}"

    # Build optional --at flag for reproducible snapshots
    local at_flag=()
    if [[ -n "${SNAPSHOT_AT_BLOCK}" ]]; then
        at_flag=(--at "${SNAPSHOT_AT_BLOCK}")
        log_info "At block:  ${SNAPSHOT_AT_BLOCK}"
    fi

    echo ""

    export RUST_LOG="${RUST_LOG:-remote-ext=debug,runtime=debug}"

    # Common migration flags: see COMMON_MIGRATION_FLAGS definition at top.
    # NOTE: --checks=all is required because try-runtime v0.10.1 does not
    #   support --checks=none. The migration runs as a side effect; the
    #   snapshot captures PRE-migration state fetched from the live chain.
    #
    # --at (optional): pins the snapshot to a specific block hash for
    #   reproducibility across CI runs.

    local start_time exit_code
    start_time=$(date +%s)

    if [[ "${DRY_RUN}" == "1" ]]; then
        log_warn "[DRY RUN] Would execute:"
        echo "  ${TRY_RUNTIME_BIN} \\"
        echo "    --runtime ${wasm_path} \\"
        echo "    on-runtime-upgrade \\"
        echo "    --blocktime ${blocktime} \\"
        echo_migration_flags
        echo "    live --uri ${uri} ${at_flag[*]+"${at_flag[*]}"} --create-snapshot ${output_path}"
        touch "${output_path}"
        exit_code=0
    else
        "${TRY_RUNTIME_BIN}" \
            --runtime "${wasm_path}" \
            on-runtime-upgrade \
            --blocktime "${blocktime}" \
            "${COMMON_MIGRATION_FLAGS[@]}" \
            live --uri "${uri}" "${at_flag[@]+"${at_flag[@]}"}" --create-snapshot "${output_path}" \
            && exit_code=0 || exit_code=$?
    fi

    local elapsed=$(( $(date +%s) - start_time ))

    echo ""
    if [[ ${exit_code} -ne 0 ]]; then
        log_error "${name}: Snapshot creation failed (exit code ${exit_code}, ${elapsed}s)"
        return ${exit_code}
    fi

    if [[ ! -f "${output_path}" ]]; then
        log_error "Snapshot file was not created: ${output_path}"
        return 1
    fi

    local snap_size
    snap_size=$(du -h "${output_path}" | cut -f1)
    log_success "${name}: Snapshot created (${snap_size}, ${elapsed}s)"
    log_info "  Path: ${output_path}"
}

create_snapshot_rootchain_testnet() {
    create_snapshot \
        "rootchain-testnet" \
        "${ROOTCHAIN_TESTNET_WASM}" \
        "${ROOTCHAIN_TESTNET_URI}" \
        "${BLOCKTIME_ROOT}"
}

create_snapshot_rootchain_mainnet() {
    create_snapshot \
        "rootchain-mainnet" \
        "${ROOTCHAIN_MAINNET_WASM}" \
        "${ROOTCHAIN_MAINNET_URI}" \
        "${BLOCKTIME_ROOT}"
}

create_snapshot_leafchain_sand_testnet() {
    create_snapshot \
        "leafchain-sand-testnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_SAND_TESTNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

create_snapshot_leafchain_avatect_mainnet() {
    create_snapshot \
        "leafchain-avatect-mainnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_AVATECT_MAINNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

create_snapshot_leafchain_lmt_testnet() {
    create_snapshot \
        "leafchain-lmt-testnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_LMT_TESTNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

create_snapshot_leafchain_lmt_mainnet() {
    create_snapshot \
        "leafchain-lmt-mainnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_LMT_MAINNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

create_snapshot_leafchain_ecq_testnet() {
    create_snapshot \
        "leafchain-ecq-testnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_ECQ_TESTNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

create_snapshot_leafchain_ecq_mainnet() {
    create_snapshot \
        "leafchain-ecq-mainnet" \
        "${LEAFCHAIN_WASM}" \
        "${LEAFCHAIN_ECQ_MAINNET_URI}" \
        "${BLOCKTIME_LEAF}"
}

create_snapshot_all_testnet() {
    log_step "Creating snapshots for all TESTNET chains"

    local failed=0

    create_snapshot_rootchain_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Rootchain testnet snapshot failed — aborting remaining chains"
        return 1
    fi

    create_snapshot_leafchain_sand_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Sand snapshot failed"
        return 1
    fi

    create_snapshot_leafchain_lmt_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain LMT testnet snapshot failed"
        return 1
    fi

    create_snapshot_leafchain_ecq_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain ECQ testnet snapshot failed"
        return 1
    fi

    log_success "All testnet snapshots created"
}

create_snapshot_all() {
    log_step "Creating snapshots for ALL chains (testnet first, then mainnet)"

    local failed=0

    # Testnet first (lower risk)
    create_snapshot_rootchain_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Rootchain testnet snapshot failed — aborting"
        return 1
    fi

    create_snapshot_leafchain_sand_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Sand snapshot failed — aborting"
        return 1
    fi

    create_snapshot_leafchain_lmt_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain LMT testnet snapshot failed — aborting"
        return 1
    fi

    create_snapshot_leafchain_ecq_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain ECQ testnet snapshot failed — aborting"
        return 1
    fi

    log_success "Testnet snapshots created, proceeding to mainnet..."
    echo ""

    # Mainnet (higher risk, run after testnet succeeds)
    create_snapshot_rootchain_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Rootchain mainnet snapshot failed — aborting"
        return 1
    fi

    create_snapshot_leafchain_avatect_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Avatect snapshot failed"
        return 1
    fi

    create_snapshot_leafchain_lmt_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain LMT mainnet snapshot failed"
        return 1
    fi

    create_snapshot_leafchain_ecq_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain ECQ mainnet snapshot failed"
        return 1
    fi

    echo ""
    log_success "All snapshots created in ${SNAPSHOT_DIR}/"
}

# ─── Snapshot Utilities ──────────────────────────────────────────────────────

list_snapshots() {
    log_step "Snapshots in ${SNAPSHOT_DIR}"

    if [[ ! -d "${SNAPSHOT_DIR}" ]]; then
        log_warn "Snapshot directory does not exist: ${SNAPSHOT_DIR}"
        return 0
    fi

    local count=0
    # List .snap files sorted by modification time (newest first).
    # SAFETY: Count matching files first. On GNU systems, piping empty
    # find output to xargs -0 ls -t may invoke ls with no args (listing CWD).
    local snap_match_count
    snap_match_count=$(find "${SNAPSHOT_DIR}" -maxdepth 1 -name '*.snap' 2>/dev/null | wc -l | tr -d ' ')

    if [[ "${snap_match_count}" -gt 0 ]]; then
        while IFS= read -r -d '' snap_file; do
            local snap_name snap_size snap_date
            snap_name=$(basename "${snap_file}")
            snap_size=$(du -h "${snap_file}" | cut -f1)
            snap_date=$(date -r "${snap_file}" "+%Y-%m-%d %H:%M:%S" 2>/dev/null \
                        || stat -c '%y' "${snap_file}" 2>/dev/null \
                        || echo "unknown")
            printf "  %-45s %8s  %s\n" "${snap_name}" "${snap_size}" "${snap_date}"
            count=$((count + 1))
        done < <(find "${SNAPSHOT_DIR}" -maxdepth 1 -name '*.snap' -print0 \
                 | xargs -0 ls -t 2>/dev/null \
                 | tr '\n' '\0')
    fi

    echo ""
    if [[ ${count} -eq 0 ]]; then
        log_info "No snapshot files found"
    else
        log_info "${count} snapshot(s) found"
    fi
}

clean_snapshots() {
    log_step "Cleaning snapshots older than ${SNAPSHOT_MAX_AGE_HOURS}h"

    if [[ ! -d "${SNAPSHOT_DIR}" ]]; then
        log_warn "Snapshot directory does not exist: ${SNAPSHOT_DIR}"
        return 0
    fi

    # Convert hours to minutes for find -mmin
    local max_age_min=$(( SNAPSHOT_MAX_AGE_HOURS * 60 ))

    local count=0
    while IFS= read -r -d '' snap_file; do
        local snap_name snap_size
        snap_name=$(basename "${snap_file}")
        snap_size=$(du -h "${snap_file}" | cut -f1)
        log_info "Removing: ${snap_name} (${snap_size})"
        rm -f "${snap_file}"
        count=$((count + 1))
    done < <(find "${SNAPSHOT_DIR}" -maxdepth 1 -name '*.snap' -mmin +"${max_age_min}" -print0)

    echo ""
    if [[ ${count} -eq 0 ]]; then
        log_info "No snapshots older than ${SNAPSHOT_MAX_AGE_HOURS}h found"
    else
        log_success "Removed ${count} snapshot(s)"
    fi

    # ── Size guard: force-delete all snapshots if directory exceeds threshold ──
    # This is a safety valve against disk accumulation when age-based cleanup
    # is insufficient (e.g., many fresh snapshots from parallel runs).
    local max_size_bytes=$(( SNAPSHOT_MAX_SIZE_GB * 1073741824 ))  # GB → bytes
    local dir_size_bytes
    dir_size_bytes=$(get_dir_size_bytes "${SNAPSHOT_DIR}")
    local dir_size_gb=$(( dir_size_bytes / 1073741824 ))

    if [[ "${dir_size_bytes}" -gt "${max_size_bytes}" ]]; then
        log_warn "Snapshot directory is ${dir_size_gb}GB (exceeds ${SNAPSHOT_MAX_SIZE_GB}GB threshold)"
        log_warn "Force-deleting ALL snapshots to reclaim disk space"

        local force_count=0
        while IFS= read -r -d '' snap_file; do
            local snap_name snap_size
            snap_name=$(basename "${snap_file}")
            snap_size=$(du -h "${snap_file}" | cut -f1)
            log_info "Force-removing: ${snap_name} (${snap_size})"
            rm -f "${snap_file}"
            force_count=$((force_count + 1))
        done < <(find "${SNAPSHOT_DIR}" -maxdepth 1 -name '*.snap' -print0)

        if [[ ${force_count} -gt 0 ]]; then
            log_warn "Force-removed ${force_count} snapshot(s) due to size threshold"
        fi

        local after_size_bytes
        after_size_bytes=$(get_dir_size_bytes "${SNAPSHOT_DIR}")
        local after_size_gb=$(( after_size_bytes / 1073741824 ))
        log_info "Snapshot directory now: ${after_size_gb}GB"
    else
        log_info "Snapshot directory size: ${dir_size_gb}GB (threshold: ${SNAPSHOT_MAX_SIZE_GB}GB)"
    fi
}

# ─── Test from Snapshot ──────────────────────────────────────────────────────
#
# Runs on-runtime-upgrade against a pre-existing snapshot file (created by
# create-snapshot or the idempotency test). This enables offline, repeatable
# migration testing without network access to live chain RPC endpoints.
#
# The snapshot captures pre-migration state, so each run starts from the
# exact same input — useful for CI caching, bisecting failures, and sharing
# reproducible test artifacts across team members.

# resolve_snapshot <chain-name>
#   Finds the most recently modified snapshot file matching <chain-name>-*.snap
#   in SNAPSHOT_DIR. Prints the absolute path to stdout. Returns 1 if none found.
resolve_snapshot() {
    local chain_name="$1"

    if [[ ! -d "${SNAPSHOT_DIR}" ]]; then
        log_error "Snapshot directory does not exist: ${SNAPSHOT_DIR}"
        return 1
    fi

    # Find the most recently modified .snap file matching this chain name.
    # SAFETY: Count matching files first. Piping empty find output to
    # xargs -0 ls -t may invoke ls with no args on GNU systems (listing
    # CWD instead of nothing). We avoid that by short-circuiting on zero matches.
    local latest=""
    local match_count
    match_count=$(find "${SNAPSHOT_DIR}" -maxdepth 1 -name "${chain_name}-*.snap" 2>/dev/null | wc -l | tr -d ' ')

    if [[ "${match_count}" -gt 0 ]]; then
        latest=$(find "${SNAPSHOT_DIR}" -maxdepth 1 -name "${chain_name}-*.snap" -print0 \
                 | xargs -0 ls -t 2>/dev/null \
                 | head -n 1)
    fi

    if [[ -z "${latest}" ]]; then
        log_error "No snapshot found matching '${chain_name}-*.snap' in ${SNAPSHOT_DIR}"
        return 1
    fi

    echo "${latest}"
}

test_from_snapshot() {
    local name="$1"
    local wasm_path="$2"
    local snapshot_path="$3"
    local blocktime="$4"

    log_step "Test from snapshot: ${name}"
    log_info "WASM:     ${wasm_path}"
    log_info "Snapshot: ${snapshot_path}"
    log_info "Blocktime: ${blocktime}ms"

    check_wasm_exists "${wasm_path}" "${name}" || return 1

    # Validate snapshot file exists
    if [[ ! -f "${snapshot_path}" ]]; then
        log_error "Snapshot file not found: ${snapshot_path}"
        return 1
    fi

    # Report snapshot size and age
    local snap_size snap_mod_epoch now_epoch age_seconds age_display
    snap_size=$(du -h "${snapshot_path}" | cut -f1)
    # macOS stat -f %m, Linux stat -c %Y — try both
    snap_mod_epoch=$(stat -f '%m' "${snapshot_path}" 2>/dev/null \
                     || stat -c '%Y' "${snapshot_path}" 2>/dev/null \
                     || echo "0")
    now_epoch=$(date +%s)
    age_seconds=$(( now_epoch - snap_mod_epoch ))

    if [[ ${age_seconds} -ge 86400 ]]; then
        age_display="$(( age_seconds / 86400 ))d $(( (age_seconds % 86400) / 3600 ))h"
    elif [[ ${age_seconds} -ge 3600 ]]; then
        age_display="$(( age_seconds / 3600 ))h $(( (age_seconds % 3600) / 60 ))m"
    else
        age_display="$(( age_seconds / 60 ))m"
    fi

    log_info "Size: ${snap_size}, Age: ${age_display}"
    echo ""

    export RUST_LOG="${RUST_LOG:-remote-ext=debug,runtime=debug}"

    # Run migration against the snapshot file (offline — no network required).
    # Common migration flags: see COMMON_MIGRATION_FLAGS definition at top.
    # Non-pipe command → use && exit_code=0 || exit_code=$? pattern.

    local start_time exit_code
    start_time=$(date +%s)

    if [[ "${DRY_RUN}" == "1" ]]; then
        log_warn "[DRY RUN] Would execute:"
        echo "  ${TRY_RUNTIME_BIN} \\"
        echo "    --runtime ${wasm_path} \\"
        echo "    on-runtime-upgrade \\"
        echo "    --blocktime ${blocktime} \\"
        echo_migration_flags
        echo "    snap --path ${snapshot_path}"
        exit_code=0
    else
        "${TRY_RUNTIME_BIN}" \
            --runtime "${wasm_path}" \
            on-runtime-upgrade \
            --blocktime "${blocktime}" \
            "${COMMON_MIGRATION_FLAGS[@]}" \
            snap --path "${snapshot_path}" \
            && exit_code=0 || exit_code=$?
    fi

    local elapsed=$(( $(date +%s) - start_time ))

    echo ""
    if [[ ${exit_code} -eq 0 ]]; then
        log_success "${name}: ALL MIGRATIONS PASSED from snapshot (${elapsed}s)"
    else
        log_error "${name}: MIGRATION FAILED from snapshot (exit code ${exit_code}, ${elapsed}s)"
        return ${exit_code}
    fi
}

test_from_snapshot_rootchain_testnet() {
    local snapshot_path="${1:-}"

    if [[ -z "${snapshot_path}" ]]; then
        snapshot_path=$(resolve_snapshot "rootchain-testnet") || return 1
    fi

    test_from_snapshot \
        "rootchain-testnet" \
        "${ROOTCHAIN_TESTNET_WASM}" \
        "${snapshot_path}" \
        "${BLOCKTIME_ROOT}"
}

test_from_snapshot_rootchain_mainnet() {
    local snapshot_path="${1:-}"

    if [[ -z "${snapshot_path}" ]]; then
        snapshot_path=$(resolve_snapshot "rootchain-mainnet") || return 1
    fi

    test_from_snapshot \
        "rootchain-mainnet" \
        "${ROOTCHAIN_MAINNET_WASM}" \
        "${snapshot_path}" \
        "${BLOCKTIME_ROOT}"
}

test_from_snapshot_leafchain_sand_testnet() {
    local snapshot_path="${1:-}"

    if [[ -z "${snapshot_path}" ]]; then
        snapshot_path=$(resolve_snapshot "leafchain-sand-testnet") || return 1
    fi

    test_from_snapshot \
        "leafchain-sand-testnet" \
        "${LEAFCHAIN_WASM}" \
        "${snapshot_path}" \
        "${BLOCKTIME_LEAF}"
}

test_from_snapshot_leafchain_avatect_mainnet() {
    local snapshot_path="${1:-}"

    if [[ -z "${snapshot_path}" ]]; then
        snapshot_path=$(resolve_snapshot "leafchain-avatect-mainnet") || return 1
    fi

    test_from_snapshot \
        "leafchain-avatect-mainnet" \
        "${LEAFCHAIN_WASM}" \
        "${snapshot_path}" \
        "${BLOCKTIME_LEAF}"
}

test_from_snapshot_leafchain_lmt_testnet() {
    local snapshot_path="${1:-}"

    if [[ -z "${snapshot_path}" ]]; then
        snapshot_path=$(resolve_snapshot "leafchain-lmt-testnet") || return 1
    fi

    test_from_snapshot \
        "leafchain-lmt-testnet" \
        "${LEAFCHAIN_WASM}" \
        "${snapshot_path}" \
        "${BLOCKTIME_LEAF}"
}

test_from_snapshot_leafchain_lmt_mainnet() {
    local snapshot_path="${1:-}"

    if [[ -z "${snapshot_path}" ]]; then
        snapshot_path=$(resolve_snapshot "leafchain-lmt-mainnet") || return 1
    fi

    test_from_snapshot \
        "leafchain-lmt-mainnet" \
        "${LEAFCHAIN_WASM}" \
        "${snapshot_path}" \
        "${BLOCKTIME_LEAF}"
}

test_from_snapshot_leafchain_ecq_testnet() {
    local snapshot_path="${1:-}"

    if [[ -z "${snapshot_path}" ]]; then
        snapshot_path=$(resolve_snapshot "leafchain-ecq-testnet") || return 1
    fi

    test_from_snapshot \
        "leafchain-ecq-testnet" \
        "${LEAFCHAIN_WASM}" \
        "${snapshot_path}" \
        "${BLOCKTIME_LEAF}"
}

test_from_snapshot_leafchain_ecq_mainnet() {
    local snapshot_path="${1:-}"

    if [[ -z "${snapshot_path}" ]]; then
        snapshot_path=$(resolve_snapshot "leafchain-ecq-mainnet") || return 1
    fi

    test_from_snapshot \
        "leafchain-ecq-mainnet" \
        "${LEAFCHAIN_WASM}" \
        "${snapshot_path}" \
        "${BLOCKTIME_LEAF}"
}

test_from_snapshot_all_testnet() {
    log_step "Running snapshot tests against all TESTNET chains"

    local failed=0

    test_from_snapshot_rootchain_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Rootchain testnet snapshot test failed — aborting remaining tests"
        return 1
    fi

    test_from_snapshot_leafchain_sand_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Sand snapshot test failed"
        return 1
    fi

    test_from_snapshot_leafchain_lmt_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain LMT testnet snapshot test failed"
        return 1
    fi

    test_from_snapshot_leafchain_ecq_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain ECQ testnet snapshot test failed"
        return 1
    fi

    log_success "All testnet snapshot tests passed"
}

test_from_snapshot_all() {
    log_step "Running snapshot tests against ALL chains (testnet first, then mainnet)"

    local failed=0

    # Testnet first (lower risk)
    test_from_snapshot_rootchain_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Rootchain testnet snapshot test failed — aborting"
        return 1
    fi

    test_from_snapshot_leafchain_sand_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Sand snapshot test failed — aborting"
        return 1
    fi

    test_from_snapshot_leafchain_lmt_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain LMT testnet snapshot test failed — aborting"
        return 1
    fi

    test_from_snapshot_leafchain_ecq_testnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain ECQ testnet snapshot test failed — aborting"
        return 1
    fi

    log_success "Testnet snapshot tests passed, proceeding to mainnet..."
    echo ""

    # Mainnet (higher risk, run after testnet passes)
    test_from_snapshot_rootchain_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Rootchain mainnet snapshot test failed — aborting"
        return 1
    fi

    test_from_snapshot_leafchain_avatect_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain Avatect snapshot test failed"
        return 1
    fi

    test_from_snapshot_leafchain_lmt_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain LMT mainnet snapshot test failed"
        return 1
    fi

    test_from_snapshot_leafchain_ecq_mainnet || failed=1
    if [[ ${failed} -ne 0 ]]; then
        log_error "Leafchain ECQ mainnet snapshot test failed"
        return 1
    fi

    echo ""
    log_step "ALL SNAPSHOT TESTS PASSED"
}

# ─── Pallet-Level Testing ────────────────────────────────────────────────────
#
# Provides pallet-focused migration analysis. Since try-runtime's
# on-runtime-upgrade executes ALL migrations (no per-pallet isolation),
# the approach is:
#   1. Run the full on-runtime-upgrade with captured output
#   2. Post-process the log to extract pallet-specific migration results
#   3. Report per-pallet: migrations executed, weight consumed, pass/fail
#
# This composes with the same try-runtime invocation flags used by
# run_try_runtime, but adds tee-to-file and grep-based output filtering.

# run_try_runtime_captured <name> <wasm_path> <uri_or_snapshot> <blocktime> <logfile>
#   Runs on-runtime-upgrade, capturing all output (stdout+stderr) to <logfile>.
#   Detects whether <uri_or_snapshot> is a file (snap --path) or URI (live --uri).
#   Returns the try-runtime exit code.
run_try_runtime_captured() {
    local name="$1"
    local wasm_path="$2"
    local uri_or_snapshot="$3"
    local blocktime="$4"
    local logfile="$5"

    check_wasm_exists "${wasm_path}" "${name}" || return 1

    export RUST_LOG="${RUST_LOG:-remote-ext=debug,runtime=debug}"

    # Determine source subcommand: file on disk = snapshot, otherwise = live URI
    local source_args=()
    if [[ -f "${uri_or_snapshot}" ]]; then
        source_args=(snap --path "${uri_or_snapshot}")
    elif [[ "${uri_or_snapshot}" =~ ^wss?:// ]]; then
        source_args=(live --uri "${uri_or_snapshot}")
    else
        log_error "Cannot determine source type for: ${uri_or_snapshot}"
        log_error "  Expected a file path (for snapshot) or ws:// / wss:// URI (for live)"
        return 1
    fi

    # Use PIPESTATUS to capture the try-runtime exit code through the tee pipe.
    # Same pattern as test_idempotency (Turn 1).
    local exit_code
    if [[ "${DRY_RUN}" == "1" ]]; then
        log_warn "[DRY RUN] Would execute:"
        echo "  ${TRY_RUNTIME_BIN} \\"
        echo "    --runtime ${wasm_path} \\"
        echo "    on-runtime-upgrade \\"
        echo "    --blocktime ${blocktime} \\"
        echo_migration_flags
        echo "    ${source_args[*]}"
        echo "  2>&1 | tee ${logfile}"
        touch "${logfile}"
        exit_code=0
    else
        set +e
        "${TRY_RUNTIME_BIN}" \
            --runtime "${wasm_path}" \
            on-runtime-upgrade \
            --blocktime "${blocktime}" \
            "${COMMON_MIGRATION_FLAGS[@]}" \
            "${source_args[@]}" \
            2>&1 | tee "${logfile}"
        exit_code=${PIPESTATUS[0]}
        set -e
    fi

    return ${exit_code}
}

# extract_pallet_output <logfile> <pallet_name>
#   Filters a try-runtime log for lines relevant to a specific pallet.
#   Prints matching lines to stdout. Returns 0 always (grep || true).
#   Searches case-insensitively for:
#     - The pallet name (underscore form, e.g., nomination_pools)
#     - The PascalCase form (e.g., NominationPools)
#     - Common migration log patterns combined with the pallet name
extract_pallet_output() {
    local logfile="$1"
    local pallet_name="$2"

    # Guard: if log file doesn't exist (e.g., early failure), produce no output
    if [[ ! -f "${logfile}" ]]; then
        return 0
    fi

    # Convert underscore_case to PascalCase for matching runtime log output.
    # e.g., nomination_pools -> NominationPools, xcmp_queue -> XcmpQueue
    local pascal_name
    pascal_name=$(echo "${pallet_name}" | awk -F'_' '{for(i=1;i<=NF;i++) $i=toupper(substr($i,1,1)) substr($i,2)} 1' OFS='')

    # Match either form, case-insensitive, to catch all relevant log lines
    grep -iE "(${pallet_name}|${pascal_name})" "${logfile}" || true
}

# report_pallet_results <logfile> <pallet_name>
#   Analyzes the captured log for a single pallet and prints a structured report.
#   Returns 0 if the pallet appears healthy, 1 if problems detected.
report_pallet_results() {
    local logfile="$1"
    local pallet_name="$2"

    # Convert to PascalCase for display and matching
    local pascal_name
    pascal_name=$(echo "${pallet_name}" | awk -F'_' '{for(i=1;i<=NF;i++) $i=toupper(substr($i,1,1)) substr($i,2)} 1' OFS='')

    echo ""
    echo "  ┌─── ${pascal_name} (${pallet_name}) ───"

    local pallet_lines
    pallet_lines=$(extract_pallet_output "${logfile}" "${pallet_name}")

    if [[ -z "${pallet_lines}" ]]; then
        echo "  │ No output lines matched this pallet"
        echo "  │ (Pallet may have no migrations, or uses a different log name)"
        echo "  └─── ${pascal_name}: SKIPPED (no output)"
        return 0
    fi

    # Count migration-related lines
    local migration_count version_count weight_count error_count
    migration_count=$(echo "${pallet_lines}" | grep -ciE '(migrat|upgrade|executed)' || true)
    version_count=$(echo "${pallet_lines}" | grep -ciE '(version|v[0-9])' || true)
    weight_count=$(echo "${pallet_lines}" | grep -ciE '(weight|consumed)' || true)
    error_count=$(echo "${pallet_lines}" | grep -ciE '(error|fail|panic|fatal)' || true)

    # Show relevant lines (indent for readability)
    while IFS= read -r line; do
        echo "  │ ${line}"
    done <<< "${pallet_lines}"

    echo "  │"
    echo "  │ Summary: migrations=${migration_count} versions=${version_count} weights=${weight_count} errors=${error_count}"

    if [[ ${error_count} -gt 0 ]]; then
        echo "  └─── ${pascal_name}: ERRORS DETECTED"
        return 1
    else
        echo "  └─── ${pascal_name}: OK"
        return 0
    fi
}

# test_pallet <pallet_name> <chain_name> <wasm_path> <uri_or_snapshot> <blocktime>
#   Tests a single pallet's storage migrations by:
#     1. Running full on-runtime-upgrade (all pallets)
#     2. Filtering output for the specified pallet
#     3. Reporting pallet-specific migration results
test_pallet() {
    local pallet_name="$1"
    local chain_name="$2"
    local wasm_path="$3"
    local uri_or_snapshot="$4"
    local blocktime="$5"

    # Convert to PascalCase for display
    local pascal_name
    pascal_name=$(echo "${pallet_name}" | awk -F'_' '{for(i=1;i<=NF;i++) $i=toupper(substr($i,1,1)) substr($i,2)} 1' OFS='')

    log_step "Pallet test: ${pascal_name} on ${chain_name}"
    log_info "Pallet:    ${pallet_name}"
    log_info "Chain:     ${chain_name}"
    log_info "WASM:      ${wasm_path}"
    log_info "Source:    ${uri_or_snapshot}"
    log_info "Blocktime: ${blocktime}ms"
    echo ""

    mkdir -p "${SNAPSHOT_DIR}"
    local logfile="${SNAPSHOT_DIR}/${chain_name}-pallet-${pallet_name}-$(date +%Y%m%d-%H%M%S).log"

    local start_time exit_code
    start_time=$(date +%s)

    log_info "Running full on-runtime-upgrade (output captured to log)..."
    run_try_runtime_captured "${chain_name}" "${wasm_path}" "${uri_or_snapshot}" "${blocktime}" "${logfile}" \
        && exit_code=0 || exit_code=$?

    local elapsed=$(( $(date +%s) - start_time ))

    echo ""
    if [[ ${exit_code} -ne 0 ]]; then
        log_error "on-runtime-upgrade FAILED (exit code ${exit_code}, ${elapsed}s)"
        log_info "Full log: ${logfile}"
        echo ""
        log_info "Extracting ${pascal_name}-specific output from failed run:"
        report_pallet_results "${logfile}" "${pallet_name}" || true
        return ${exit_code}
    fi

    log_success "on-runtime-upgrade passed (${elapsed}s)"
    log_info "Extracting ${pascal_name}-specific results..."

    local pallet_exit=0
    report_pallet_results "${logfile}" "${pallet_name}" || pallet_exit=1

    echo ""
    log_info "Full log: ${logfile}"

    if [[ ${pallet_exit} -eq 0 ]]; then
        log_success "${pascal_name} on ${chain_name}: PALLET TEST PASSED"
    else
        log_error "${pascal_name} on ${chain_name}: PALLET TEST FOUND ERRORS"
        return 1
    fi
}

# test_pallet_batch <chain_name> <wasm_path> <uri_or_snapshot> <blocktime> <pallet_name...>
#   Tests multiple pallets efficiently: runs on-runtime-upgrade ONCE, then
#   filters output for EACH pallet. Much faster than N separate invocations.
test_pallet_batch() {
    local chain_name="$1"
    local wasm_path="$2"
    local uri_or_snapshot="$3"
    local blocktime="$4"
    shift 4
    local pallets=("$@")

    if [[ ${#pallets[@]} -eq 0 ]]; then
        log_error "No pallets specified for batch test"
        return 1
    fi

    log_step "Pallet batch test: ${#pallets[@]} pallets on ${chain_name}"
    log_info "Chain:     ${chain_name}"
    log_info "WASM:      ${wasm_path}"
    log_info "Source:    ${uri_or_snapshot}"
    log_info "Blocktime: ${blocktime}ms"
    log_info "Pallets:   ${pallets[*]}"
    echo ""

    mkdir -p "${SNAPSHOT_DIR}"
    local logfile="${SNAPSHOT_DIR}/${chain_name}-pallet-batch-$(date +%Y%m%d-%H%M%S).log"

    local start_time exit_code
    start_time=$(date +%s)

    log_info "Running full on-runtime-upgrade (single invocation for all pallets)..."
    run_try_runtime_captured "${chain_name}" "${wasm_path}" "${uri_or_snapshot}" "${blocktime}" "${logfile}" \
        && exit_code=0 || exit_code=$?

    local elapsed=$(( $(date +%s) - start_time ))

    echo ""
    if [[ ${exit_code} -ne 0 ]]; then
        log_error "on-runtime-upgrade FAILED (exit code ${exit_code}, ${elapsed}s)"
    else
        log_success "on-runtime-upgrade passed (${elapsed}s)"
    fi

    log_info "Analyzing per-pallet results..."
    echo ""

    local total=${#pallets[@]}
    local passed=0
    local failed=0
    local skipped=0

    for pallet in "${pallets[@]}"; do
        local pallet_result=0
        report_pallet_results "${logfile}" "${pallet}" || pallet_result=1

        if [[ ${pallet_result} -ne 0 ]]; then
            failed=$((failed + 1))
        else
            # Distinguish between OK and SKIPPED by checking if any lines matched
            local match_count
            match_count=$(extract_pallet_output "${logfile}" "${pallet}" | wc -l | tr -d ' ')
            if [[ ${match_count} -eq 0 ]]; then
                skipped=$((skipped + 1))
            else
                passed=$((passed + 1))
            fi
        fi
    done

    echo ""
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║                  PALLET BATCH RESULTS                      ║"
    echo "╠══════════════════════════════════════════════════════════════╣"
    printf "║  Chain:    %-47s ║\n" "${chain_name}"
    printf "║  Total:    %-47s ║\n" "${total} pallets"
    printf "║  Passed:   %-47s ║\n" "${passed}"
    printf "║  Skipped:  %-47s ║\n" "${skipped} (no output matched)"
    printf "║  Errors:   %-47s ║\n" "${failed}"
    printf "║  Runtime:  %-47s ║\n" "${exit_code} (exit code), ${elapsed}s"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""
    log_info "Full log: ${logfile}"

    if [[ ${exit_code} -ne 0 ]]; then
        log_error "on-runtime-upgrade itself failed — all pallet results are suspect"
        return ${exit_code}
    fi

    if [[ ${failed} -gt 0 ]]; then
        log_error "${failed} pallet(s) had errors in their output"
        return 1
    fi

    log_success "Pallet batch test: ALL ${total} pallets OK on ${chain_name}"
}

# ─── Pallet Migration Matrix ─────────────────────────────────────────────────
#
# Runs on-runtime-upgrade ONCE per chain, then filters output for each pallet,
# producing a chain x pallet grid with OK/SKIP/ERROR for each cell.
#
# This is the CI-optimized view: O(chains) try-runtime invocations, not
# O(chains x pallets). The matrix provides granular per-pallet-per-chain
# visibility without redundant work.
#
# Environment variables:
#   MATRIX_CHAINS   Space-separated list of chain names (overrides arguments)
#   MATRIX_PALLETS  Space-separated list of pallet names (default: KNOWN_PALLETS)

# abbreviate_chain <chain-name>
#   Produces a short column header for the matrix display.
#   e.g., rootchain-testnet → rc-test, leafchain-avatect-mainnet → lc-avat
abbreviate_chain() {
    local chain="$1"
    case "${chain}" in
        rootchain-testnet)          echo "rc-test" ;;
        rootchain-mainnet)          echo "rc-main" ;;
        leafchain-sand-testnet)     echo "lc-sand" ;;
        leafchain-avatect-mainnet)  echo "lc-avat" ;;
        leafchain-lmt-testnet)      echo "lc-lmt-t" ;;
        leafchain-lmt-mainnet)      echo "lc-lmt-m" ;;
        leafchain-ecq-testnet)      echo "lc-ecq-t" ;;
        leafchain-ecq-mainnet)      echo "lc-ecq-m" ;;
        *)
            # Generic abbreviation: take first 2 chars of each hyphen-delimited part
            echo "${chain}" | awk -F'-' '{for(i=1;i<=NF;i++) printf "%s", substr($i,1,4); print ""}'
            ;;
    esac
}

# test_pallet_matrix [chain1 chain2 ...] [-- pallet1 pallet2 ...]
#   Runs on-runtime-upgrade once per chain, analyzes per-pallet results,
#   and prints a chain x pallet matrix grid.
#
#   Chain/pallet lists can also come from env vars:
#     MATRIX_CHAINS="rootchain-testnet leafchain-sand-testnet"
#     MATRIX_PALLETS="nomination_pools staking session"
#
#   If no pallets specified, uses KNOWN_PALLETS.
#   Exit code: 0 if all cells are OK or SKIP, non-zero if any ERROR.
test_pallet_matrix() {
    # Guard: declare -A (associative arrays) requires bash >= 4.
    # On macOS the system bash is 3.2 — users must install bash 5 (brew install bash).
    if [[ "${BASH_VERSINFO[0]}" -lt 4 ]]; then
        log_error "test-pallet-matrix requires bash >= 4 (for associative arrays)"
        log_error "Current bash version: ${BASH_VERSION}"
        log_info "On macOS, install bash 5 via: brew install bash"
        return 1
    fi

    # ── Parse arguments: chains [-- pallets] ────────────────────────────
    local chains=()
    local pallets=()
    local parsing_pallets=0

    for arg in "$@"; do
        if [[ "${arg}" == "--" ]]; then
            parsing_pallets=1
            continue
        fi
        if [[ ${parsing_pallets} -eq 1 ]]; then
            pallets+=("${arg}")
        else
            chains+=("${arg}")
        fi
    done

    # Override from env vars if set (env vars take precedence over args)
    if [[ -n "${MATRIX_CHAINS:-}" ]]; then
        IFS=' ' read -ra chains <<< "${MATRIX_CHAINS}"
    fi
    if [[ -n "${MATRIX_PALLETS:-}" ]]; then
        IFS=' ' read -ra pallets <<< "${MATRIX_PALLETS}"
    fi

    # Defaults
    if [[ ${#chains[@]} -eq 0 ]]; then
        log_error "No chains specified. Set MATRIX_CHAINS or pass chain names as arguments."
        echo "  Example: $0 test-pallet-matrix rootchain-testnet leafchain-sand-testnet"
        echo "  Or: MATRIX_CHAINS='rootchain-testnet leafchain-sand-testnet' $0 test-pallet-matrix"
        return 1
    fi
    if [[ ${#pallets[@]} -eq 0 ]]; then
        pallets=("${KNOWN_PALLETS[@]}")
    fi

    local num_chains=${#chains[@]}
    local num_pallets=${#pallets[@]}

    log_step "Pallet Migration Matrix: ${num_pallets} pallets x ${num_chains} chains"
    log_info "Chains:  ${chains[*]}"
    log_info "Pallets: ${pallets[*]}"
    echo ""

    # ── Data structures: flat associative arrays ────────────────────────
    # MATRIX[chain_idx:pallet_idx] = "OK" | "SKIP" | "ERROR"
    # CHAIN_EXIT[chain_idx] = exit_code
    # CHAIN_LOGFILE[chain_idx] = path to logfile
    declare -A MATRIX
    declare -A CHAIN_EXIT
    declare -A CHAIN_LOGFILE
    declare -A CHAIN_ELAPSED

    mkdir -p "${SNAPSHOT_DIR}"

    # ── Run on-runtime-upgrade ONCE per chain ───────────────────────────
    local ci=0
    for chain in "${chains[@]}"; do
        local _wasm_path _uri _blocktime
        resolve_chain_params "${chain}" || {
            # Mark entire column as ERROR for unrecognized chain
            CHAIN_EXIT[${ci}]=1
            CHAIN_LOGFILE[${ci}]=""
            CHAIN_ELAPSED[${ci}]=0
            local pi=0
            for _ in "${pallets[@]}"; do
                MATRIX[${ci}:${pi}]="ERROR"
                pi=$((pi + 1))
            done
            ci=$((ci + 1))
            continue
        }

        local logfile="${SNAPSHOT_DIR}/${chain}-matrix-$(date +%Y%m%d-%H%M%S).log"
        CHAIN_LOGFILE[${ci}]="${logfile}"

        log_info "[${chain}] Running on-runtime-upgrade..."
        local start_time exit_code
        start_time=$(date +%s)

        run_try_runtime_captured "${chain}" "${_wasm_path}" "${_uri}" "${_blocktime}" "${logfile}" \
            && exit_code=0 || exit_code=$?

        CHAIN_EXIT[${ci}]=${exit_code}
        CHAIN_ELAPSED[${ci}]=$(( $(date +%s) - start_time ))

        if [[ ${exit_code} -ne 0 ]]; then
            log_error "[${chain}] on-runtime-upgrade FAILED (exit code ${exit_code}, ${CHAIN_ELAPSED[${ci}]}s)"
            # Mark all pallets as ERROR for this chain
            local pi=0
            for _ in "${pallets[@]}"; do
                MATRIX[${ci}:${pi}]="ERROR"
                pi=$((pi + 1))
            done
        else
            log_success "[${chain}] on-runtime-upgrade passed (${CHAIN_ELAPSED[${ci}]}s)"
            # Classify each pallet
            local pi=0
            for pallet in "${pallets[@]}"; do
                local pallet_lines
                pallet_lines=$(extract_pallet_output "${logfile}" "${pallet}")

                if [[ -z "${pallet_lines}" ]]; then
                    MATRIX[${ci}:${pi}]="SKIP"
                else
                    # Check for errors in pallet-specific output
                    local error_count
                    error_count=$(echo "${pallet_lines}" | grep -ciE '(error|fail|panic|fatal)' || true)
                    if [[ ${error_count} -gt 0 ]]; then
                        MATRIX[${ci}:${pi}]="ERROR"
                    else
                        MATRIX[${ci}:${pi}]="OK"
                    fi
                fi
                pi=$((pi + 1))
            done
        fi
        echo ""
        ci=$((ci + 1))
    done

    # ── Compute column widths ───────────────────────────────────────────
    # Pallet name column: max of pallet name lengths (min 20)
    local pcw=20   # pallet column width (content chars)
    for pallet in "${pallets[@]}"; do
        local len=${#pallet}
        if [[ ${len} -gt ${pcw} ]]; then
            pcw=${len}
        fi
    done

    # Chain column width: visible content chars (must fit "  ERROR  " = 9)
    local ccw=9

    # Build abbreviated chain headers
    local chain_headers=()
    ci=0
    for chain in "${chains[@]}"; do
        chain_headers+=("$(abbreviate_chain "${chain}")")
        ci=$((ci + 1))
    done

    # ── Print matrix ────────────────────────────────────────────────────
    #
    # Row anatomy (all visible characters between ║ and ║):
    #   ║ <pallet, pcw chars> │ <chain, ccw chars> │ <chain, ccw chars> ║
    #
    # Segment widths:
    #   pallet segment = " " + pcw chars + " " = pcw + 2
    #   chain segment  = " " + ccw chars + " " = ccw + 2  (preceded by │)
    #   After last chain: nothing extra (║ closes)
    #
    # inner_width = (pcw + 2) + num_chains * (1 + ccw + 2) - 1
    #   The -1 removes the trailing space of the last chain segment
    #   because ║ closes immediately after the last space.
    #   Actually, let's keep it: each chain is │ + space + ccw + space.
    #
    # Concrete: ║{sp}{pcw}{sp}│{sp}{ccw}{sp}│{sp}{ccw}{sp}║
    #   inner = (pcw+2) + num_chains*(ccw+3) - the │ is part of the chain segment
    #   Wait: │ is 1 char. Each chain segment = │ + sp + ccw + sp = ccw + 3.
    #   But │ takes 1 inner_width char.
    #   inner = (1 + pcw + 1) + num_chains * (1 + 1 + ccw + 1)
    #         = pcw + 2 + num_chains * (ccw + 3)
    #
    # Separator: ╟──{pcw+2}──┼──{ccw+2}──┼──{ccw+2}──╢
    #   = (pcw + 2) dashes + num_chains * (1 + ccw + 2) dashes
    #   = pcw + 2 + num_chains * (ccw + 3) = inner_width. Correct!
    local inner_width=$(( pcw + 2 + num_chains * (ccw + 3) ))

    echo ""

    # Helper: repeat a char N times
    repeat_char() {
        local char="$1" count="$2"
        if [[ ${count} -le 0 ]]; then return; fi
        printf "%${count}s" "" | tr ' ' "${char}"
    }

    # Top border
    printf "╔"; repeat_char "═" "${inner_width}"; printf "╗\n"

    # Title
    local title="PALLET MIGRATION MATRIX RESULTS"
    local title_len=${#title}
    local title_lpad=$(( (inner_width - title_len) / 2 ))
    local title_rpad=$(( inner_width - title_len - title_lpad ))
    printf "║"; repeat_char " " "${title_lpad}"
    printf "%s" "${title}"
    repeat_char " " "${title_rpad}"; printf "║\n"

    # Header separator
    printf "╠"; repeat_char "═" "${inner_width}"; printf "╣\n"

    # Column headers
    printf "║ %-${pcw}s " "Pallet"
    for header in "${chain_headers[@]}"; do
        printf "│ %-${ccw}s " "${header}"
    done
    printf "║\n"

    # Header underline
    printf "╟"; repeat_char "─" $((pcw + 2))
    for _ in "${chain_headers[@]}"; do
        printf "┼"; repeat_char "─" $((ccw + 2))
    done
    printf "╢\n"

    # Data rows
    local any_error=0
    local pi=0
    for pallet in "${pallets[@]}"; do
        printf "║ %-${pcw}s " "${pallet}"
        ci=0
        for _ in "${chains[@]}"; do
            local cell="${MATRIX[${ci}:${pi}]}"
            # Build cell: plain text centered in ccw chars, with ANSI color
            # ccw=9: "    OK   " "  SKIP   " "  ERROR  " "   ??    "
            local plain_text label_len lpad rpad
            case "${cell}" in
                OK)    plain_text="OK";    label_len=2 ;;
                SKIP)  plain_text="SKIP";  label_len=4 ;;
                ERROR) plain_text="ERROR"; label_len=5; any_error=1 ;;
                *)     plain_text="??";    label_len=2 ;;
            esac
            lpad=$(( (ccw - label_len) / 2 ))
            rpad=$(( ccw - label_len - lpad ))

            # Color selection
            local color
            case "${cell}" in
                OK)    color="${GREEN}" ;;
                SKIP)  color="${YELLOW}" ;;
                ERROR) color="${RED}" ;;
                *)     color="${CYAN}" ;;
            esac

            printf "│ "
            repeat_char " " "${lpad}"
            echo -e -n "${color}${plain_text}${NC}"
            repeat_char " " "${rpad}"
            printf " "
            ci=$((ci + 1))
        done
        printf "║\n"
        pi=$((pi + 1))
    done

    # Totals row separator
    printf "╟"; repeat_char "─" $((pcw + 2))
    for _ in "${chain_headers[@]}"; do
        printf "┼"; repeat_char "─" $((ccw + 2))
    done
    printf "╢\n"

    # Totals row: count OK / total for each chain
    printf "║ %-${pcw}s " "TOTAL"
    ci=0
    for _ in "${chains[@]}"; do
        local ok_count=0
        pi=0
        for _ in "${pallets[@]}"; do
            local cell="${MATRIX[${ci}:${pi}]}"
            if [[ "${cell}" == "OK" ]]; then
                ok_count=$((ok_count + 1))
            fi
            pi=$((pi + 1))
        done
        local total_str="${ok_count}/${num_pallets}"
        local total_len=${#total_str}
        local total_lpad=$(( (ccw - total_len) / 2 ))
        local total_rpad=$(( ccw - total_len - total_lpad ))
        printf "│ "
        repeat_char " " "${total_lpad}"
        printf "%s" "${total_str}"
        repeat_char " " "${total_rpad}"
        printf " "
        ci=$((ci + 1))
    done
    printf "║\n"

    # Bottom border
    printf "╚"; repeat_char "═" "${inner_width}"; printf "╝\n"

    echo ""

    # ── Per-chain timing summary ────────────────────────────────────────
    ci=0
    for chain in "${chains[@]}"; do
        local exit_c="${CHAIN_EXIT[${ci}]}"
        local elapsed_c="${CHAIN_ELAPSED[${ci}]}"
        if [[ ${exit_c} -eq 0 ]]; then
            log_success "[${chain}] exit=${exit_c}, elapsed=${elapsed_c}s"
        else
            log_error "[${chain}] exit=${exit_c}, elapsed=${elapsed_c}s"
        fi
        if [[ -n "${CHAIN_LOGFILE[${ci}]}" ]]; then
            log_info "  Log: ${CHAIN_LOGFILE[${ci}]}"
        fi
        ci=$((ci + 1))
    done

    echo ""

    # ── Final verdict ───────────────────────────────────────────────────
    if [[ ${any_error} -ne 0 ]]; then
        log_error "MATRIX RESULT: ERRORS DETECTED in one or more cells"
        return 1
    fi

    log_success "MATRIX RESULT: ALL pallets OK or SKIP across all chains"
    return 0
}

# ─── CI Readiness Verification ───────────────────────────────────────────────
#
# Comprehensive pre-flight check that validates the ENTIRE try-runtime pipeline
# is correctly configured and ready for CI. Can be run locally or in CI.

verify_ci_readiness() {
    log_step "CI Readiness Verification"

    local total_checks=0
    local passed_checks=0
    local warned_checks=0
    local failed_checks=0

    # Helper: record check result
    check_pass() {
        log_success "$1"
        total_checks=$((total_checks + 1))
        passed_checks=$((passed_checks + 1))
    }
    check_warn() {
        log_warn "$1"
        total_checks=$((total_checks + 1))
        warned_checks=$((warned_checks + 1))
    }
    check_fail() {
        log_error "$1"
        total_checks=$((total_checks + 1))
        failed_checks=$((failed_checks + 1))
    }

    # ── 1. Tool checks ─────────────────────────────────────────────────
    echo ""
    log_info "=== 1. Tool Checks ==="

    # 1a. try-runtime CLI
    if command -v "${TRY_RUNTIME_BIN}" &>/dev/null; then
        local version
        version="$("${TRY_RUNTIME_BIN}" --version 2>/dev/null || echo 'unknown')"
        check_pass "try-runtime CLI: ${version}"
    else
        check_fail "try-runtime CLI not found at '${TRY_RUNTIME_BIN}'"
    fi

    # 1b. bash version >= 4 (required for associative arrays in test_pallet_matrix)
    local bash_major
    bash_major="${BASH_VERSINFO[0]}"
    if [[ ${bash_major} -ge 4 ]]; then
        check_pass "bash version: ${BASH_VERSION} (>= 4, associative arrays supported)"
    else
        check_fail "bash version: ${BASH_VERSION} (< 4, associative arrays NOT supported)"
        log_info "  On macOS, install bash 5 via: brew install bash"
    fi

    # ── 2. WASM existence checks ────────────────────────────────────────
    echo ""
    log_info "=== 2. WASM Existence ==="

    local wasm_names=("rootchain-testnet" "rootchain-mainnet" "leafchain")
    local wasm_paths=("${ROOTCHAIN_TESTNET_WASM}" "${ROOTCHAIN_MAINNET_WASM}" "${LEAFCHAIN_WASM}")
    local wasm_all_found=1

    for i in 0 1 2; do
        local wn="${wasm_names[$i]}"
        local wp="${wasm_paths[$i]}"
        if [[ -f "${wp}" ]]; then
            local wsize
            wsize=$(du -h "${wp}" | cut -f1)
            check_pass "WASM ${wn}: ${wp} (${wsize})"
        else
            wasm_all_found=0
            if [[ "${SKIP_BUILD}" == "1" ]]; then
                check_fail "WASM ${wn}: NOT FOUND (SKIP_BUILD=1, expected pre-built at ${wp})"
            else
                check_warn "WASM ${wn}: not built yet (run build-all first)"
            fi
        fi
    done

    # ── 3. Endpoint reachability ────────────────────────────────────────
    echo ""
    log_info "=== 3. Endpoint Reachability ==="

    # Guard: curl is needed for endpoint probes. Skip gracefully if unavailable.
    if ! command -v curl &>/dev/null; then
        check_warn "curl not found — skipping endpoint reachability checks"
    else
        local endpoints=(
            "rootchain-testnet:${ROOTCHAIN_TESTNET_URI}"
            "rootchain-mainnet:${ROOTCHAIN_MAINNET_URI}"
            "leafchain-sand-testnet:${LEAFCHAIN_SAND_TESTNET_URI}"
            "leafchain-avatect-mainnet:${LEAFCHAIN_AVATECT_MAINNET_URI}"
            "leafchain-lmt-testnet:${LEAFCHAIN_LMT_TESTNET_URI}"
            "leafchain-lmt-mainnet:${LEAFCHAIN_LMT_MAINNET_URI}"
            "leafchain-ecq-testnet:${LEAFCHAIN_ECQ_TESTNET_URI}"
            "leafchain-ecq-mainnet:${LEAFCHAIN_ECQ_MAINNET_URI}"
        )

        for entry in "${endpoints[@]}"; do
            local ep_name="${entry%%:*}"
            local ep_uri="${entry#*:}"
            # Convert wss:// to https:// for probe, ws:// to http://
            local probe_uri="${ep_uri}"
            probe_uri="${probe_uri/wss:\/\//https://}"
            probe_uri="${probe_uri/ws:\/\//http://}"

            # Quick HTTP probe with 10s timeout — just check connectivity, not WebSocket handshake
            if curl -sSf --max-time 10 -o /dev/null "${probe_uri}" 2>/dev/null; then
                check_pass "Endpoint ${ep_name}: reachable (${ep_uri})"
            else
                # Also try with --insecure in case of self-signed cert
                if curl -sSfk --max-time 10 -o /dev/null "${probe_uri}" 2>/dev/null; then
                    check_warn "Endpoint ${ep_name}: reachable (self-signed cert) (${ep_uri})"
                else
                    check_warn "Endpoint ${ep_name}: unreachable (may require VPN) (${ep_uri})"
                fi
            fi
        done
    fi

    # ── 4. Snapshot directory ───────────────────────────────────────────
    echo ""
    log_info "=== 4. Snapshot Directory ==="

    if [[ -d "${SNAPSHOT_DIR}" ]]; then
        if [[ -w "${SNAPSHOT_DIR}" ]]; then
            check_pass "Snapshot dir exists and writable: ${SNAPSHOT_DIR}"
        else
            check_fail "Snapshot dir exists but NOT writable: ${SNAPSHOT_DIR}"
        fi

        # Check disk space available (in human-readable form)
        local disk_avail
        disk_avail=$(df -h "${SNAPSHOT_DIR}" | awk 'NR==2{print $4}')
        log_info "  Disk available: ${disk_avail}"

        # Count existing snapshots
        local snap_count
        snap_count=$(find "${SNAPSHOT_DIR}" -maxdepth 1 -name '*.snap' 2>/dev/null | wc -l | tr -d ' ')
        log_info "  Existing snapshots: ${snap_count}"
    else
        # Not a failure — it will be created on first use
        check_warn "Snapshot dir does not exist yet: ${SNAPSHOT_DIR} (will be created on first use)"
        # Check parent is writable
        local parent_dir
        parent_dir=$(dirname "${SNAPSHOT_DIR}")
        if [[ -w "${parent_dir}" ]]; then
            log_info "  Parent dir writable: ${parent_dir}"
        else
            check_fail "Parent dir NOT writable: ${parent_dir}"
        fi
    fi

    # ── 5. Script integrity (command dispatch) ──────────────────────────
    echo ""
    log_info "=== 5. Script Integrity ==="

    # Verify that known commands are handled in the case statement
    local known_commands=(
        build-rootchain build-leafchain build-all
        test-rootchain-testnet test-rootchain-mainnet
        test-leafchain-sand-testnet test-leafchain-avatect-mainnet
        test-leafchain-lmt-testnet test-leafchain-lmt-mainnet
        test-leafchain-ecq-testnet test-leafchain-ecq-mainnet
        test-all-testnet test-all
        test-idempotency-rootchain-testnet test-idempotency-rootchain-mainnet
        test-idempotency-leafchain-sand-testnet test-idempotency-leafchain-avatect-mainnet
        test-idempotency-leafchain-lmt-testnet test-idempotency-leafchain-lmt-mainnet
        test-idempotency-leafchain-ecq-testnet test-idempotency-leafchain-ecq-mainnet
        test-idempotency-all-testnet test-idempotency-all
        create-snapshot-rootchain-testnet create-snapshot-rootchain-mainnet
        create-snapshot-leafchain-sand-testnet create-snapshot-leafchain-avatect-mainnet
        create-snapshot-leafchain-lmt-testnet create-snapshot-leafchain-lmt-mainnet
        create-snapshot-leafchain-ecq-testnet create-snapshot-leafchain-ecq-mainnet
        create-snapshot-all-testnet create-snapshot-all
        list-snapshots clean-snapshots
        test-from-snapshot-rootchain-testnet test-from-snapshot-rootchain-mainnet
        test-from-snapshot-leafchain-sand-testnet test-from-snapshot-leafchain-avatect-mainnet
        test-from-snapshot-leafchain-lmt-testnet test-from-snapshot-leafchain-lmt-mainnet
        test-from-snapshot-leafchain-ecq-testnet test-from-snapshot-leafchain-ecq-mainnet
        test-from-snapshot-all-testnet test-from-snapshot-all
        test-pallet test-pallet-critical test-pallet-matrix
        check checklist verify-ci-readiness
        version help
    )

    local dispatch_ok=1
    local script_path="${BASH_SOURCE[0]}"
    for cmd in "${known_commands[@]}"; do
        # Match either standalone "cmd)" or compound "cmd|..." / "...|cmd)" patterns
        if grep -qE "^[[:space:]]*(${cmd}[|)]|[a-z|_-]*\|${cmd}[)|])" "${script_path}" 2>/dev/null; then
            : # found in case statement
        else
            check_fail "Command '${cmd}' not found in case statement dispatch"
            dispatch_ok=0
        fi
    done
    if [[ ${dispatch_ok} -eq 1 ]]; then
        check_pass "All ${#known_commands[@]} commands dispatch correctly in case statement"
    fi

    # ── 6. Known pallets ────────────────────────────────────────────────
    echo ""
    log_info "=== 6. Known Pallets ==="

    if [[ ${#KNOWN_PALLETS[@]} -gt 0 ]]; then
        check_pass "KNOWN_PALLETS array: ${#KNOWN_PALLETS[@]} pallets (${KNOWN_PALLETS[*]})"
    else
        check_fail "KNOWN_PALLETS array is empty"
    fi

    # ── 7. Configuration summary ────────────────────────────────────────
    echo ""
    log_info "=== 7. Configuration Summary ==="

    printf "\n"
    printf "  ╔══════════════════════════════════════════════════════════════════╗\n"
    printf "  ║                    CONFIGURATION TABLE                          ║\n"
    printf "  ╠══════════════════════════════════════════════════════════════════╣\n"
    printf "  ║  %-25s  %-38s ║\n" "Setting" "Value"
    printf "  ╟──────────────────────────────────────────────────────────────────╢\n"
    printf "  ║  %-25s  %-38s ║\n" "PROJECT_ROOT" "${PROJECT_ROOT}"
    printf "  ║  %-25s  %-38s ║\n" "WASM_DIR" "${WASM_DIR}"
    printf "  ║  %-25s  %-38s ║\n" "TRY_RUNTIME_BIN" "${TRY_RUNTIME_BIN}"
    printf "  ║  %-25s  %-38s ║\n" "SNAPSHOT_DIR" "${SNAPSHOT_DIR}"
    printf "  ║  %-25s  %-38s ║\n" "SKIP_BUILD" "${SKIP_BUILD}"
    printf "  ║  %-25s  %-38s ║\n" "DRY_RUN" "${DRY_RUN}"
    printf "  ║  %-25s  %-38s ║\n" "KEEP_SNAPSHOTS" "${KEEP_SNAPSHOTS}"
    printf "  ║  %-25s  %-38s ║\n" "SNAPSHOT_MAX_AGE_HOURS" "${SNAPSHOT_MAX_AGE_HOURS}"
    printf "  ║  %-25s  %-38s ║\n" "SNAPSHOT_SUPPORTED" "${SNAPSHOT_SUPPORTED}"
    printf "  ║  %-25s  %-38s ║\n" "SNAPSHOT_MAX_SIZE_GB" "${SNAPSHOT_MAX_SIZE_GB}GB"
    printf "  ║  %-25s  %-38s ║\n" "BLOCKTIME_ROOT" "${BLOCKTIME_ROOT}ms"
    printf "  ║  %-25s  %-38s ║\n" "BLOCKTIME_LEAF" "${BLOCKTIME_LEAF}ms"
    printf "  ╟──────────────────────────────────────────────────────────────────╢\n"
    printf "  ║  %-25s  %-38s ║\n" "ROOTCHAIN_TESTNET_URI" "${ROOTCHAIN_TESTNET_URI:0:38}"
    printf "  ║  %-25s  %-38s ║\n" "ROOTCHAIN_MAINNET_URI" "${ROOTCHAIN_MAINNET_URI:0:38}"
    printf "  ║  %-25s  %-38s ║\n" "LEAFCHAIN_SAND_TESTNET_URI" "${LEAFCHAIN_SAND_TESTNET_URI:0:38}"
    printf "  ║  %-25s  %-38s ║\n" "LEAFCHAIN_AVATECT_MAINNET_URI" "${LEAFCHAIN_AVATECT_MAINNET_URI:0:38}"
    printf "  ║  %-25s  %-38s ║\n" "LEAFCHAIN_LMT_TESTNET_URI" "${LEAFCHAIN_LMT_TESTNET_URI:0:38}"
    printf "  ║  %-25s  %-38s ║\n" "LEAFCHAIN_LMT_MAINNET_URI" "${LEAFCHAIN_LMT_MAINNET_URI:0:38}"
    printf "  ║  %-25s  %-38s ║\n" "LEAFCHAIN_ECQ_TESTNET_URI" "${LEAFCHAIN_ECQ_TESTNET_URI:0:38}"
    printf "  ║  %-25s  %-38s ║\n" "LEAFCHAIN_ECQ_MAINNET_URI" "${LEAFCHAIN_ECQ_MAINNET_URI:0:38}"
    printf "  ╟──────────────────────────────────────────────────────────────────╢\n"
    printf "  ║  %-25s  %-38s ║\n" "ROOTCHAIN_TESTNET_WASM" "$(basename "${ROOTCHAIN_TESTNET_WASM}")"
    printf "  ║  %-25s  %-38s ║\n" "ROOTCHAIN_MAINNET_WASM" "$(basename "${ROOTCHAIN_MAINNET_WASM}")"
    printf "  ║  %-25s  %-38s ║\n" "LEAFCHAIN_WASM" "$(basename "${LEAFCHAIN_WASM}")"
    printf "  ╟──────────────────────────────────────────────────────────────────╢\n"
    printf "  ║  %-25s  %-38s ║\n" "CARGO_TARGET_DIR" "${CARGO_TARGET_DIR:-<default>}"
    printf "  ║  %-25s  %-38s ║\n" "RUST_LOG" "${RUST_LOG:-<unset>}"
    printf "  ║  %-25s  %-38s ║\n" "SNAPSHOT_AT_BLOCK" "${SNAPSHOT_AT_BLOCK:-<unset>}"
    printf "  ║  %-25s  %-38s ║\n" "MATRIX_CHAINS" "${MATRIX_CHAINS:-<unset>}"
    printf "  ║  %-25s  %-38s ║\n" "MATRIX_PALLETS" "${MATRIX_PALLETS:-<unset>}"
    printf "  ╚══════════════════════════════════════════════════════════════════╝\n"

    # ── Summary ─────────────────────────────────────────────────────────
    echo ""
    echo "╔══════════════════════════════════════════════════════════════════╗"
    printf "║  CI READINESS: %-3d passed, %-3d warnings, %-3d failed  (%-3d total) ║\n" \
        "${passed_checks}" "${warned_checks}" "${failed_checks}" "${total_checks}"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    echo ""

    if [[ ${failed_checks} -gt 0 ]]; then
        log_error "CI readiness check FAILED (${failed_checks} critical issue(s))"
        return 1
    elif [[ ${warned_checks} -gt 0 ]]; then
        log_warn "CI readiness check passed with ${warned_checks} warning(s)"
        return 0
    else
        log_success "CI readiness check PASSED — all systems go"
        return 0
    fi
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
    echo "║  8. Idempotency verification                                   ║"
    echo "║     [ ] Both passes completed without errors                   ║"
    echo "║     [ ] Pass 1 and Pass 2 output identical key metrics         ║"
    echo "║     [ ] Deterministic: no timestamp/randomness drift           ║"
    echo "║     [ ] Weight consumed identical across both passes           ║"
    echo "║                                                                ║"
    echo "║  9. Snapshot management                                        ║"
    echo "║     [ ] Snapshot file exists and is non-empty                  ║"
    echo "║     [ ] Snapshot age < SNAPSHOT_MAX_AGE_HOURS                  ║"
    echo "║     [ ] Snapshot-based test matches live-state test results    ║"
    echo "║     [ ] Disk space adequate (snapshots can be large)           ║"
    echo "║     [ ] SNAPSHOT_AT_BLOCK used for reproducible comparisons    ║"
    echo "║                                                                ║"
    echo "║ 10. Pallet-level testing                                       ║"
    echo "║     [ ] Per-pallet results matrix shows all PASS/SKIP          ║"
    echo "║     [ ] SKIP = pallet not present on chain (expected)          ║"
    echo "║     [ ] No unexpected FAIL for any pallet x chain pair         ║"
    echo "║     [ ] Critical pallets (KNOWN_PALLETS) all PASS              ║"
    echo "║     [ ] test-pallet-matrix grid consistent across runs         ║"
    echo "║                                                                ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    echo ""
}

# ─── Version ─────────────────────────────────────────────────────────────────

SCRIPT_VERSION="2.0.0"

show_version() {
    echo "try-runtime-test.sh v${SCRIPT_VERSION} (Wave 2+3 enhanced)"
    echo ""
    echo "Capabilities:"
    echo "  - Basic migration testing (live + snapshot)"
    echo "  - Idempotency verification (deterministic dual-pass)"
    echo "  - Snapshot management (create, list, clean, auto-discover)"
    echo "  - Pallet-level analysis (single, batch, matrix)"
    echo "  - CI integration (DRY_RUN, verify-ci-readiness)"
    echo ""
    echo "Snapshot support: SNAPSHOT_SUPPORTED=${SNAPSHOT_SUPPORTED}"
    if [[ "${SNAPSHOT_SUPPORTED}" != "true" ]]; then
        echo "  (--create-snapshot not available in installed try-runtime)"
    fi
}

# ─── Help ────────────────────────────────────────────────────────────────────

show_help() {
    echo "Usage:"
    echo "  ./scripts/try-runtime-test.sh [COMMAND]"
    echo ""
    echo "Build commands:"
    echo "  build-rootchain                  Build rootchain runtimes (mainnet + testnet)"
    echo "  build-leafchain                  Build leafchain (general-runtime)"
    echo "  build-all                        Build all runtimes"
    echo ""
    echo "Migration test commands:"
    echo "  test-rootchain-testnet           Run migrations against rootchain testnet"
    echo "  test-rootchain-mainnet           Run migrations against rootchain mainnet"
    echo "  test-leafchain-sand-testnet              Run migrations against leafchain Sand"
    echo "  test-leafchain-avatect-mainnet           Run migrations against leafchain Avatect"
    echo "  test-leafchain-lmt-testnet       Run migrations against leafchain LMT testnet"
    echo "  test-leafchain-lmt-mainnet       Run migrations against leafchain LMT mainnet"
    echo "  test-leafchain-ecq-testnet       Run migrations against leafchain ECQ testnet"
    echo "  test-leafchain-ecq-mainnet       Run migrations against leafchain ECQ mainnet"
    echo "  test-all-testnet                 Run all testnet chains"
    echo "  test-all                         Run all chains (testnet first, then mainnet)"
    echo ""
    echo "Idempotency test commands:"
    echo "  test-idempotency-rootchain-testnet   Dual-pass deterministic test: rootchain testnet"
    echo "  test-idempotency-rootchain-mainnet   Dual-pass deterministic test: rootchain mainnet"
    echo "  test-idempotency-leafchain-sand-testnet      Dual-pass deterministic test: leafchain Sand"
    echo "  test-idempotency-leafchain-avatect-mainnet       Dual-pass deterministic test: leafchain Avatect"
    echo "  test-idempotency-leafchain-lmt-testnet Dual-pass deterministic test: leafchain LMT testnet"
    echo "  test-idempotency-leafchain-lmt-mainnet Dual-pass deterministic test: leafchain LMT mainnet"
    echo "  test-idempotency-leafchain-ecq-testnet Dual-pass deterministic test: leafchain ECQ testnet"
    echo "  test-idempotency-leafchain-ecq-mainnet Dual-pass deterministic test: leafchain ECQ mainnet"
    echo "  test-idempotency-all-testnet             Dual-pass deterministic test: all testnet chains"
    echo "  test-idempotency-all                 Dual-pass deterministic test: all chains"
    echo ""
    echo "Snapshot commands:"
    echo "  create-snapshot-rootchain-testnet     Save rootchain testnet state as snapshot"
    echo "  create-snapshot-rootchain-mainnet     Save rootchain mainnet state as snapshot"
    echo "  create-snapshot-leafchain-sand-testnet        Save leafchain Sand state as snapshot"
    echo "  create-snapshot-leafchain-avatect-mainnet         Save leafchain Avatect state as snapshot"
    echo "  create-snapshot-leafchain-lmt-testnet   Save leafchain LMT testnet state as snapshot"
    echo "  create-snapshot-leafchain-lmt-mainnet   Save leafchain LMT mainnet state as snapshot"
    echo "  create-snapshot-leafchain-ecq-testnet   Save leafchain ECQ testnet state as snapshot"
    echo "  create-snapshot-leafchain-ecq-mainnet   Save leafchain ECQ mainnet state as snapshot"
    echo "  create-snapshot-all-testnet               Save all testnet chain snapshots"
    echo "  create-snapshot-all                   Save all chain snapshots"
    echo "  list-snapshots                        List all saved snapshots (name, size, date)"
    echo "  clean-snapshots                       Remove snapshots older than SNAPSHOT_MAX_AGE_HOURS"
    echo ""
    echo "  test-from-snapshot-rootchain-testnet [path]   Test from snapshot: rootchain testnet"
    echo "  test-from-snapshot-rootchain-mainnet [path]   Test from snapshot: rootchain mainnet"
    echo "  test-from-snapshot-leafchain-sand-testnet [path]      Test from snapshot: leafchain Sand"
    echo "  test-from-snapshot-leafchain-avatect-mainnet [path]       Test from snapshot: leafchain Avatect"
    echo "  test-from-snapshot-leafchain-lmt-testnet [path] Test from snapshot: leafchain LMT testnet"
    echo "  test-from-snapshot-leafchain-lmt-mainnet [path] Test from snapshot: leafchain LMT mainnet"
    echo "  test-from-snapshot-leafchain-ecq-testnet [path] Test from snapshot: leafchain ECQ testnet"
    echo "  test-from-snapshot-leafchain-ecq-mainnet [path] Test from snapshot: leafchain ECQ mainnet"
    echo "  test-from-snapshot-all-testnet                    Test all testnet chains from snapshots"
    echo "  test-from-snapshot-all                        Test all chains from snapshots"
    echo ""
    echo "Pallet-level test commands:"
    echo "  test-pallet <pallet> <chain>         Test single pallet migration on a chain"
    echo "  test-pallet-critical <chain>          Test all critical pallets (KNOWN_PALLETS)"
    echo "  test-pallet-matrix [chains...] [-- pallets...]"
    echo "                                        Matrix test: pallets x chains grid"
    echo "                                        Also reads MATRIX_CHAINS / MATRIX_PALLETS"
    echo ""
    echo "  Valid chains: rootchain-testnet, rootchain-mainnet, leafchain-sand-testnet, leafchain-avatect-mainnet, leafchain-lmt-testnet, leafchain-lmt-mainnet, leafchain-ecq-testnet, leafchain-ecq-mainnet"
    echo ""
    echo "Utility commands:"
    echo "  verify-ci-readiness              Comprehensive pre-flight check for CI pipeline"
    echo "  check                            Verify try-runtime CLI and WASM artifacts"
    echo "  checklist                        Print post-run verification checklist"
    echo "  version                          Print script version and capabilities"
    echo "  help                             Show this help"
    echo ""
    echo "Environment variables:"
    echo "  CARGO_TARGET_DIR           Override cargo target directory (default: ./target)"
    echo "  TRY_RUNTIME_BIN            Path to try-runtime binary (default: try-runtime in PATH)"
    echo "  ROOTCHAIN_TESTNET_URI      Rootchain testnet RPC endpoint"
    echo "  ROOTCHAIN_MAINNET_URI      Rootchain mainnet RPC endpoint"
    echo "  LEAFCHAIN_SAND_TESTNET_URI         Leafchain Sand RPC endpoint"
    echo "  LEAFCHAIN_AVATECT_MAINNET_URI      Leafchain Avatect RPC endpoint"
    echo "  LEAFCHAIN_LMT_TESTNET_URI  Leafchain LMT testnet RPC endpoint"
    echo "  LEAFCHAIN_LMT_MAINNET_URI  Leafchain LMT mainnet RPC endpoint"
    echo "  LEAFCHAIN_ECQ_TESTNET_URI  Leafchain ECQ testnet RPC endpoint"
    echo "  LEAFCHAIN_ECQ_MAINNET_URI  Leafchain ECQ mainnet RPC endpoint"
    echo "  BLOCKTIME_ROOT             Rootchain block time in ms (default: 6000)"
    echo "  BLOCKTIME_LEAF             Leafchain block time in ms (default: 12000)"
    echo "  SKIP_BUILD                 Set to 1 to skip WASM build (use pre-built artifacts)"
    echo "  DRY_RUN                    Set to 1 to print commands without executing"
    echo "  SNAPSHOT_DIR               Snapshot file directory (default: target/try-runtime-snapshots)"
    echo "  KEEP_SNAPSHOTS             Set to 1 to preserve snapshot files after tests"
    echo "  SNAPSHOT_AT_BLOCK          Pin snapshot to a specific block hash (reproducible)"
    echo "  SNAPSHOT_MAX_AGE_HOURS     Max snapshot age in hours for clean-snapshots (default: 24)"
    echo "  SNAPSHOT_MAX_SIZE_GB       Max total snapshot dir size in GB; force-deletes all if exceeded (default: 50)"
    echo "  MATRIX_CHAINS              Space-separated chain list for test-pallet-matrix"
    echo "  MATRIX_PALLETS             Space-separated pallet list for test-pallet-matrix"
    echo "                             (default: KNOWN_PALLETS)"
    echo ""
    print_verification_checklist
}

# ─── Snapshot Capability Gate (dispatch helper) ─────────────────────────────
# Called at dispatch entry points for snapshot-dependent commands.
# Provides immediate, clear feedback before any setup or function-level work.
# The function-level gates in create_snapshot() and test_idempotency() are
# defense-in-depth — this gate is the user-facing early exit.

# require_snapshot_support <command-name>
#   Returns 0 if --create-snapshot is available (caller should proceed).
#   Returns 1 if not available (caller should skip). Prints a warning.
#   Safe under set -e when used as: require_snapshot_support "cmd" || return 0
require_snapshot_support() {
    local cmd_name="$1"
    if [[ "${SNAPSHOT_SUPPORTED}" != "true" ]]; then
        log_warn "Command '${cmd_name}' requires --create-snapshot support"
        log_warn "The installed try-runtime CLI does not support --create-snapshot"
        log_info "  SNAPSHOT_SUPPORTED=${SNAPSHOT_SUPPORTED}"
        log_info "  Upgrade try-runtime CLI to enable this command"
        return 1
    fi
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
        test-leafchain-sand-testnet)      test_leafchain_sand_testnet ;;
        test-leafchain-avatect-mainnet)       test_leafchain_avatect_mainnet ;;
        test-leafchain-lmt-testnet)   test_leafchain_lmt_testnet ;;
        test-leafchain-lmt-mainnet)   test_leafchain_lmt_mainnet ;;
        test-leafchain-ecq-testnet)   test_leafchain_ecq_testnet ;;
        test-leafchain-ecq-mainnet)   test_leafchain_ecq_mainnet ;;
        test-all-testnet)         test_all_testnet ;;
        test-all)                 test_all ;;
        # ── Snapshot-dependent commands (gated by SNAPSHOT_SUPPORTED) ──────
        # These commands require --create-snapshot, which is not available in
        # try-runtime v0.10.1. The dispatch-level gate exits early with a clear
        # warning; the function-level gates are defense-in-depth.
        test-idempotency-rootchain-testnet)
            require_snapshot_support "${cmd}" || exit 0
            test_idempotency_rootchain_testnet ;;
        test-idempotency-rootchain-mainnet)
            require_snapshot_support "${cmd}" || exit 0
            test_idempotency_rootchain_mainnet ;;
        test-idempotency-leafchain-sand-testnet)
            require_snapshot_support "${cmd}" || exit 0
            test_idempotency_leafchain_sand_testnet ;;
        test-idempotency-leafchain-avatect-mainnet)
            require_snapshot_support "${cmd}" || exit 0
            test_idempotency_leafchain_avatect_mainnet ;;
        test-idempotency-leafchain-lmt-testnet)
            require_snapshot_support "${cmd}" || exit 0
            test_idempotency_leafchain_lmt_testnet ;;
        test-idempotency-leafchain-lmt-mainnet)
            require_snapshot_support "${cmd}" || exit 0
            test_idempotency_leafchain_lmt_mainnet ;;
        test-idempotency-leafchain-ecq-testnet)
            require_snapshot_support "${cmd}" || exit 0
            test_idempotency_leafchain_ecq_testnet ;;
        test-idempotency-leafchain-ecq-mainnet)
            require_snapshot_support "${cmd}" || exit 0
            test_idempotency_leafchain_ecq_mainnet ;;
        test-idempotency-all-testnet)
            require_snapshot_support "${cmd}" || exit 0
            test_idempotency_all_testnet ;;
        test-idempotency-all)
            require_snapshot_support "${cmd}" || exit 0
            test_idempotency_all ;;
        create-snapshot-rootchain-testnet)
            require_snapshot_support "${cmd}" || exit 0
            create_snapshot_rootchain_testnet ;;
        create-snapshot-rootchain-mainnet)
            require_snapshot_support "${cmd}" || exit 0
            create_snapshot_rootchain_mainnet ;;
        create-snapshot-leafchain-sand-testnet)
            require_snapshot_support "${cmd}" || exit 0
            create_snapshot_leafchain_sand_testnet ;;
        create-snapshot-leafchain-avatect-mainnet)
            require_snapshot_support "${cmd}" || exit 0
            create_snapshot_leafchain_avatect_mainnet ;;
        create-snapshot-leafchain-lmt-testnet)
            require_snapshot_support "${cmd}" || exit 0
            create_snapshot_leafchain_lmt_testnet ;;
        create-snapshot-leafchain-lmt-mainnet)
            require_snapshot_support "${cmd}" || exit 0
            create_snapshot_leafchain_lmt_mainnet ;;
        create-snapshot-leafchain-ecq-testnet)
            require_snapshot_support "${cmd}" || exit 0
            create_snapshot_leafchain_ecq_testnet ;;
        create-snapshot-leafchain-ecq-mainnet)
            require_snapshot_support "${cmd}" || exit 0
            create_snapshot_leafchain_ecq_mainnet ;;
        create-snapshot-all-testnet)
            require_snapshot_support "${cmd}" || exit 0
            create_snapshot_all_testnet ;;
        create-snapshot-all)
            require_snapshot_support "${cmd}" || exit 0
            create_snapshot_all ;;
        list-snapshots)                       list_snapshots ;;
        clean-snapshots)                      clean_snapshots ;;
        test-from-snapshot-rootchain-testnet)  test_from_snapshot_rootchain_testnet "${2:-}" ;;
        test-from-snapshot-rootchain-mainnet)  test_from_snapshot_rootchain_mainnet "${2:-}" ;;
        test-from-snapshot-leafchain-sand-testnet)     test_from_snapshot_leafchain_sand_testnet "${2:-}" ;;
        test-from-snapshot-leafchain-avatect-mainnet)      test_from_snapshot_leafchain_avatect_mainnet "${2:-}" ;;
        test-from-snapshot-leafchain-lmt-testnet)  test_from_snapshot_leafchain_lmt_testnet "${2:-}" ;;
        test-from-snapshot-leafchain-lmt-mainnet)  test_from_snapshot_leafchain_lmt_mainnet "${2:-}" ;;
        test-from-snapshot-leafchain-ecq-testnet)  test_from_snapshot_leafchain_ecq_testnet "${2:-}" ;;
        test-from-snapshot-leafchain-ecq-mainnet)  test_from_snapshot_leafchain_ecq_mainnet "${2:-}" ;;
        test-from-snapshot-all-testnet)        test_from_snapshot_all_testnet ;;
        test-from-snapshot-all)                test_from_snapshot_all ;;
        test-pallet)
            # test-pallet <pallet_name> <chain>
            local pallet_arg="${2:-}"
            local chain_arg="${3:-}"
            if [[ -z "${pallet_arg}" || -z "${chain_arg}" ]]; then
                log_error "Usage: $0 test-pallet <pallet_name> <chain>"
                echo "  Example: $0 test-pallet nomination_pools rootchain-testnet"
                echo "  Chains: rootchain-testnet, rootchain-mainnet, leafchain-sand-testnet, leafchain-avatect-mainnet, leafchain-lmt-testnet, leafchain-lmt-mainnet, leafchain-ecq-testnet, leafchain-ecq-mainnet"
                exit 1
            fi
            local _wasm_path _uri _blocktime
            resolve_chain_params "${chain_arg}" || exit 1
            test_pallet "${pallet_arg}" "${chain_arg}" "${_wasm_path}" "${_uri}" "${_blocktime}"
            ;;
        test-pallet-critical)
            # test-pallet-critical <chain>
            local chain_arg="${2:-}"
            if [[ -z "${chain_arg}" ]]; then
                log_error "Usage: $0 test-pallet-critical <chain>"
                echo "  Example: $0 test-pallet-critical rootchain-testnet"
                echo "  Chains: rootchain-testnet, rootchain-mainnet, leafchain-sand-testnet, leafchain-avatect-mainnet, leafchain-lmt-testnet, leafchain-lmt-mainnet, leafchain-ecq-testnet, leafchain-ecq-mainnet"
                exit 1
            fi
            local _wasm_path _uri _blocktime
            resolve_chain_params "${chain_arg}" || exit 1
            test_pallet_batch "${chain_arg}" "${_wasm_path}" "${_uri}" "${_blocktime}" "${KNOWN_PALLETS[@]}"
            ;;
        test-pallet-matrix)
            # test-pallet-matrix [chain1 chain2 ...] [-- pallet1 pallet2 ...]
            # Also reads MATRIX_CHAINS and MATRIX_PALLETS env vars
            shift
            test_pallet_matrix "$@"
            ;;
        verify-ci-readiness)      verify_ci_readiness ;;
        check)                    do_check ;;
        checklist)                print_verification_checklist ;;
        version|--version|-V)     show_version ;;
        help|--help|-h)           show_help ;;
        *)
            log_error "Unknown command: ${cmd}"
            echo "Run '$0 help' for usage."
            exit 1
            ;;
    esac
}

main "$@"
