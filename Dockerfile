FROM rust:1.59.0 AS chef 
RUN cargo install cargo-chef 
WORKDIR app

FROM chef AS planner
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

RUN apt-get update && apt-get install -y musl-tools curl llvm clang ca-certificates

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
