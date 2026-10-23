#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
set -a; . thxnet/ci/versions.env; set +a
toml_list() { sed -n "s/^$1 *= *\[\(.*\)\]/\1/p" rust-toolchain.toml | tr -d '" ' | tr ',' ' '; }
RUST_TOOLCHAIN=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)
RUST_COMPONENTS=$(toml_list components)
RUST_TARGETS=$(toml_list targets)
test -n "$RUST_TOOLCHAIN" && test -n "$RUST_TARGETS"
fp=$(cat thxnet/docker/ci/builder.Dockerfile thxnet/ci/versions.env rust-toolchain.toml | sha256sum | cut -c1-12)
tag="$BUILDER_IMAGE_NAME:$RUST_TOOLCHAIN-$fp"
case ${1:-tag} in
  tag) echo "$tag" ;;
  build)
    docker image inspect "$tag" >/dev/null 2>&1 && { echo "$tag"; exit 0; }
    docker build -f thxnet/docker/ci/builder.Dockerfile \
      --build-arg RUST_TOOLCHAIN="$RUST_TOOLCHAIN" --build-arg RUST_COMPONENTS="$RUST_COMPONENTS" --build-arg RUST_TARGETS="$RUST_TARGETS" --build-arg SCCACHE_VERSION="$SCCACHE_VERSION" --build-arg SCCACHE_SHA256="$SCCACHE_SHA256" \
      -t "$tag" thxnet/docker/ci >&2
    echo "$tag" ;;
  *) echo "usage: builder-image.sh [tag|build]" >&2; exit 2 ;;
esac
