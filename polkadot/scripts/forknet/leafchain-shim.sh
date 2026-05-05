#!/usr/bin/env bash
# Shim that translates fork-genesis's call to old `export-genesis-state` into
# v1.12.0 leafchain binary's `export-genesis-head`. Otherwise pass through.
REAL=/mnt/HC_Volume_105402799/worktrees/thxnet-release-v1.12-test/ci-artefacts/binaries/thxnet-leafchain
if [[ "${1:-}" == "export-genesis-state" ]]; then
    shift
    exec "$REAL" export-genesis-head "$@"
fi
exec "$REAL" "$@"
