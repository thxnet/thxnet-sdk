#!/usr/bin/env bash
set -euo pipefail
: "${THX_CACHE_ROOT:?}" "${THX_TARGET_DIR:?}"
cd "$(git rev-parse --show-toplevel)"
set -a; . thxnet/ci/cache.env; set +a
img=$(thxnet/ci/builder-image.sh build)
mkdir -p "$THX_CACHE_ROOT/cargo-registry" "$THX_CACHE_ROOT/cargo-git" "$THX_CACHE_ROOT/sccache" "$THX_TARGET_DIR"
exec docker run --rm --cpu-shares 512 -v "$PWD:/src" -w /src \
  -v "$THX_CACHE_ROOT/cargo-registry:/usr/local/cargo/registry" -v "$THX_CACHE_ROOT/cargo-git:/usr/local/cargo/git" \
  -v "$THX_CACHE_ROOT/sccache:/sccache" -v "$THX_TARGET_DIR:/target" \
  -e CARGO_TARGET_DIR=/target -e SCCACHE_DIR=/sccache -e RUSTC_WRAPPER=sccache -e CARGO_INCREMENTAL=0 \
  -e SCCACHE_CACHE_SIZE -e SUBSTRATE_CLI_GIT_COMMIT_HASH="$(git rev-parse --short=11 HEAD)" \
  ${CARGO_BUILD_JOBS:+-e CARGO_BUILD_JOBS} \
  "$img" bash -c 'git config --global --add safe.directory /src; sccache --start-server >/dev/null; s=$(date +%s); "$@"; rc=$?; echo "elapsed_sec=$(( $(date +%s) - s )) rc=$rc"; sccache --show-stats; exit $rc' _ "$@"
