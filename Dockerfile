FROM rust:1.59.0 AS chef 
RUN cargo install cargo-chef 
WORKDIR app

FROM chef AS planner
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# need to build ssl
RUN apt-get update && apt-get install -y musl-tools curl llvm clang ca-certificates
ENV OPENSSL_VERSION="1_1_1f"
ENV OPENSSL_FILE="OpenSSL_${OPENSSL_VERSION}.tar.gz"
# Note: https://github.com/openssl/openssl/issues/7207#issuecomment-880121450
RUN wget -q https://github.com/openssl/openssl/archive/${OPENSSL_FILE} && \
  sha256sum  ${OPENSSL_FILE} && \
  echo "76b78352bc8a9aaccc1de2eb4a52fa9c7f6a5980985242ce3514b0cd208642d3  ${OPENSSL_FILE}" | sha256sum -c - && \
  tar zxvf ${OPENSSL_FILE} && \
  cd openssl-*/ && \
  export CC="musl-gcc -fPIE -pie -static -idirafter /usr/include/ -idirafter /usr/include/x86_64-linux-gnu/" && \
 ./Configure no-shared no-async --prefix=/musl --openssldir=/musl/ssl linux-x86_64 && \
  make depend && make -j$(nproc) && make install

ENV PKG_CONFIG_ALLOW_CROSS=1
ENV OPENSSL_STATIC=true
ENV OPENSSL_DIR=/musl

# Build dependencies - this is the caching Docker layer!
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl --bin chain-monitor

FROM scratch AS runtime
WORKDIR app
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/chain-monitor /usr/local/bin/
COPY --from=builder /etc/ssl/certs /etc/ssl/certs
ENTRYPOINT ["/usr/local/bin/chain-monitor", "-l", "3000", "--enable-prometheus"]
EXPOSE 3000
