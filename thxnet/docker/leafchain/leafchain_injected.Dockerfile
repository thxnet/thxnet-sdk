# Lightweight Dockerfile that copies pre-built binaries.
# Used by CI after building binaries in a separate step.
# For a from-source build, see leafchain_builder.Dockerfile.
FROM docker.io/library/ubuntu:20.04

LABEL description="Container image for THXNET. leafchain" \
	io.thxnet.image.type="final" \
	io.thxnet.image.authors="contact@thxlab.io" \
	io.thxnet.image.vendor="thxlab.io" \
	io.thxnet.image.description="THXNET.: The Hybrid Next-Gen Blockchain Network"

COPY target/release/thxnet-leafchain /usr/local/bin

RUN useradd -m -u 1000 -U -s /bin/sh -d /leafchain thxnet && \
	mkdir -p /data /leafchain/.local/share && \
	chown -R thxnet:thxnet /data && \
	rm -rf /usr/bin /usr/sbin && \
	/usr/local/bin/thxnet-leafchain --version

USER thxnet

EXPOSE 30333 60002 9615
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/thxnet-leafchain"]
