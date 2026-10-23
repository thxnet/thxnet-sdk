#!/usr/bin/env bash
set -euo pipefail
: "${THX_CACHE_ROOT:?}" "${THX_TARGET_DIR:?}"
cd "$(git rev-parse --show-toplevel)"
set -a; . thxnet/ci/cache.env; set +a
img=$(thxnet/ci/builder-image.sh build)
mkdir -p "$THX_CACHE_ROOT/cargo-home" "$THX_CACHE_ROOT/sccache" "$THX_TARGET_DIR"
name="thx-cargo-$(date +%s)-$$"
stop() { docker stop -t 10 "$name" >/dev/null 2>&1 || true; }
trap stop INT TERM
docker run --rm --name "$name" --label "thx.ci.run=${GITHUB_RUN_ID:-local}" --cpu-shares 512 --user "$(id -u):$(id -g)" -e HOME=/tmp -v "$PWD:/src" -w /src \
  -v "$THX_CACHE_ROOT/cargo-home:/cargo-home" -v "$THX_CACHE_ROOT/sccache:/sccache" -v "$THX_TARGET_DIR:/target" \
  -e CARGO_HOME=/cargo-home -e CARGO_TARGET_DIR=/target -e WASM_BUILD_WORKSPACE_HINT=/src -e SCCACHE_DIR=/sccache -e RUSTC_WRAPPER=sccache -e CARGO_INCREMENTAL=0 \
  -e SCCACHE_CACHE_SIZE -e SUBSTRATE_CLI_GIT_COMMIT_HASH="$(git rev-parse --short=11 HEAD)" \
  ${CARGO_BUILD_JOBS:+-e CARGO_BUILD_JOBS} \
  "$img" bash -c 'git config --global --add safe.directory /src; sccache --start-server >/dev/null; s=$(date +%s); "$@"; rc=$?; echo "elapsed_sec=$(( $(date +%s) - s )) rc=$rc"; sccache --show-stats; exit $rc' _ "$@" &
pid=$!
wait "$pid"; rc=$?
while kill -0 "$pid" 2>/dev/null; do wait "$pid"; rc=$?; done
exit $rc
