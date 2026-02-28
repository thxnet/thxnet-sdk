#!/usr/bin/env bash
# Wrapper to run nix-built polkadot binary outside of nix develop shell.
# The binary links against nix store glibc/libz, so we invoke it via the nix ld-linux.
#
# v1.1.0 also spawns PVF worker subprocesses (polkadot-prepare-worker, polkadot-execute-worker).
# These inherit LD_LIBRARY_PATH and their ELF interpreter points to nix ld-linux, so they work
# as long as --workers-path points to the directory containing the actual worker binaries.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"

NIX_LD="/nix/store/pf5avvvl4ssd6kylcvg2g23hcjp71h19-glibc-2.39-52/lib/ld-linux-x86-64.so.2"
NIX_LIBS="/nix/store/f2q5ld1nipl8w1r2w8m6azhlm2varqgb-zlib-1.3.1/lib:/nix/store/90yn7340r8yab8kxpb0p7y0c9j3snjam-gcc-13.2.0-lib/lib"
POLKADOT="${REPO_DIR}/target/release/polkadot"
WORKERS_PATH="${REPO_DIR}/target/release"

# Only inject --workers-path for node operation (not for utility subcommands like build-spec)
EXTRA_ARGS=()
case "${1:-}" in
  build-spec|export-genesis-state|export-genesis-wasm|--version|--help|-V|-h)
    ;;
  *)
    EXTRA_ARGS=("--workers-path=${WORKERS_PATH}")
    ;;
esac

exec env LD_LIBRARY_PATH="${NIX_LIBS}" "${NIX_LD}" "${POLKADOT}" "${EXTRA_ARGS[@]}" "$@"
