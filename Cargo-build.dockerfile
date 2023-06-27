# Build environment setup
# examples/envoy/docker-compose.yaml shows how to mount the build directory
# as a volume so the build artifacts will be cached locally
FROM rust:1.70.0 as builder

# The user ID and group ID of the host user
# so the container can be run as the host user to avoid permission issues
ARG USER_ID
ARG GROUP_ID

# Create a new user with the host user's ID and group ID
# This enables mounting the project directory as a volume for build caching
RUN groupadd -g $GROUP_ID user && useradd -l -u $USER_ID -g user user
USER user

WORKDIR /build

# Install wasm32-wasi target to build for WASM
RUN rustup target add wasm32-wasi
