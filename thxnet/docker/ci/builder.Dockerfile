FROM docker.io/library/ubuntu:24.04

ARG RUST_TOOLCHAIN
ARG RUST_COMPONENTS
ARG RUST_TARGETS
ARG SCCACHE_VERSION
ARG SCCACHE_SHA256

RUN test -n "$RUST_TOOLCHAIN" && test -n "$RUST_TARGETS" && test -n "$SCCACHE_VERSION" && test -n "$SCCACHE_SHA256"

RUN apt-get update && \
	DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
		ca-certificates curl git jq make cmake pkg-config clang libclang-dev lld \
		protobuf-compiler libssl-dev && \
	rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
	CARGO_HOME=/usr/local/cargo \
	PATH=/usr/local/cargo/bin:$PATH

RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal --default-toolchain "$RUST_TOOLCHAIN" && \
	rustup component add $RUST_COMPONENTS && \
	rustup target add $RUST_TARGETS && \
	rustc --version

RUN f="sccache-${SCCACHE_VERSION}-x86_64-unknown-linux-musl" && \
	curl -sSfL -o /tmp/s.tgz "https://github.com/mozilla/sccache/releases/download/${SCCACHE_VERSION}/${f}.tar.gz" && \
	echo "${SCCACHE_SHA256}  /tmp/s.tgz" | sha256sum -c - && \
	tar -xzf /tmp/s.tgz -C /tmp && install -m 0755 "/tmp/${f}/sccache" /usr/local/bin/sccache && \
	rm -rf /tmp/s.tgz "/tmp/${f}" && sccache --version

LABEL io.thxnet.image.type="ci-builder" \
	io.thxnet.image.vendor="thxlab.io"
