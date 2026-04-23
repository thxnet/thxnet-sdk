# This is the build stage for THXNET. leafchain. Here we create the binary in a temporary image.
FROM ci-linux as builder

# Stamp the binary with the exact commit hash. Required — without it,
# build-script-utils falls back to "unknown" when .git is absent from the
# build context, producing untraceable binaries. Callers MUST pass this
# via --build-arg (docker-bake.hcl handles this from host env).
ARG SUBSTRATE_CLI_GIT_COMMIT_HASH
ENV SUBSTRATE_CLI_GIT_COMMIT_HASH=$SUBSTRATE_CLI_GIT_COMMIT_HASH

WORKDIR /build
COPY . /build

RUN test -n "$SUBSTRATE_CLI_GIT_COMMIT_HASH" || { \
      echo "ERROR: SUBSTRATE_CLI_GIT_COMMIT_HASH not set. Pass it via --build-arg." >&2; \
      exit 1; \
    }

RUN cargo build --locked --release -p thxnet-leafchain

# This is the 2nd stage: a very small image where we copy the THXNET. binary.
FROM ubuntu as leafchain

LABEL description="Container image for THXNET. leafchain" \
	io.thxnet.image.type="builder" \
	io.thxnet.image.authors="contact@thxlab.io" \
	io.thxnet.image.vendor="thxlab.io" \
	io.thxnet.image.description="THXNET.: The Hybrid Next-Gen Blockchain Network"

COPY --from=builder /build/target/release/thxnet-leafchain /usr/local/bin

RUN useradd -m -u 1000 -U -s /bin/sh -d /leafchain thxnet && \
	mkdir -p /data /leafchain/.local/share && \
	chown -R thxnet:thxnet /data && \
	rm -rf /usr/bin /usr/sbin && \
	/usr/local/bin/thxnet-leafchain --version

USER thxnet

EXPOSE 30333 60002 9615
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/thxnet-leafchain"]
