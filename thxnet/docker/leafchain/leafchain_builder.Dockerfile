# This is the build stage for THXNET. leafchain. Here we create the binary in a temporary image.
FROM ci-linux as builder

WORKDIR /build
COPY . /build

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
