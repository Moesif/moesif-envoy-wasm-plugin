# First stage: build the WASM binary
FROM rust:1.70.0 as builder

ARG USER_ID
ARG GROUP_ID

# Create a new user with the host user's ID and group ID
RUN groupadd -g $GROUP_ID user && useradd -l -u $USER_ID -g user user
USER user

WORKDIR /build

# Install wasm32-wasi target
RUN rustup target add wasm32-wasi

# Copy project files
COPY --chown=user:user . .

# Build the WASM binary
RUN cargo build --release --target=wasm32-wasi

CMD ["echo", "Build complete!"]

# # Second stage: setup runtime environment
# FROM envoyproxy/envoy:v1.25.6

# # Copy built binary from builder stage
# COPY --from=builder /build/target/wasm32-wasi/release /etc/envoy/proxy-wasm-plugins

# COPY ./envoy.yaml /etc/envoy/envoy.yaml

# CMD ["envoy", "-c", "/etc/envoy/envoy.yaml", "--concurrency", "1", "--log-level", "debug"]
