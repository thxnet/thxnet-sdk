#!/usr/bin/env bash
# Wrapper to run nix-built thxnet-leafchain binary outside of nix develop shell.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"

NIX_LD="/nix/store/pf5avvvl4ssd6kylcvg2g23hcjp71h19-glibc-2.39-52/lib/ld-linux-x86-64.so.2"
NIX_LIBS="/nix/store/f2q5ld1nipl8w1r2w8m6azhlm2varqgb-zlib-1.3.1/lib:/nix/store/90yn7340r8yab8kxpb0p7y0c9j3snjam-gcc-13.2.0-lib/lib"
LEAFCHAIN="${REPO_DIR}/target/release/thxnet-leafchain"

exec env LD_LIBRARY_PATH="${NIX_LIBS}" "${NIX_LD}" "${LEAFCHAIN}" "$@"
