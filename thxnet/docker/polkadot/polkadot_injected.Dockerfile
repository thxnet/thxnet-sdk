# Lightweight Dockerfile that copies pre-built binaries.
# Used by CI after building binaries in a separate step.
# For a from-source build, see polkadot_builder.Dockerfile.
FROM docker.io/library/ubuntu:24.04

LABEL description="Container image for THXNET." \
	io.thxnet.image.type="final" \
	io.thxnet.image.authors="contact@thxlab.io" \
	io.thxnet.image.vendor="thxlab.io" \
	io.thxnet.image.description="THXNET.: The Hybrid Next-Gen Blockchain Network"

COPY target/release/polkadot /usr/local/bin
COPY target/release/polkadot-prepare-worker /usr/local/bin
COPY target/release/polkadot-execute-worker /usr/local/bin

RUN apt-get update && \
	DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates && \
	rm -rf /var/lib/apt/lists/* && \
	(userdel -r ubuntu 2>/dev/null || true) && \
	useradd -m -u 1000 -U -s /bin/sh -d /rootchain thxnet && \
	mkdir -p /data /rootchain/.local/share && \
	chown -R thxnet:thxnet /data && \
	ln -s /data /rootchain/.local/share/polkadot && \
	rm -rf /usr/bin /usr/sbin && \
	/usr/local/bin/polkadot --version

USER thxnet

EXPOSE 30333 9933 9944 9615
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/polkadot"]
