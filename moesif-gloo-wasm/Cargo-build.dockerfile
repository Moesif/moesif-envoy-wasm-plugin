# First stage: build the WASM binary
FROM rust:1.70.0 as builder

# Install wasm32-wasi target
RUN rustup target add wasm32-wasi

WORKDIR /build/moesif-gloo-wasm

COPY . .

# Build the WASM binary
RUN cargo build --target=wasm32-wasi --release 

# Second stage: setup runtime environment
FROM envoyproxy/envoy:v1.25.6

# Copy built binary from builder stage
COPY --from=builder /build/moesif-gloo-wasm/target/wasm32-wasi/release /etc/envoy/proxy-wasm-plugins

COPY ./envoy.yaml /etc/envoy/envoy.yaml

CMD ["envoy", "-c", "/etc/envoy/envoy.yaml", "--concurrency", "1", "--log-level", "debug"]
