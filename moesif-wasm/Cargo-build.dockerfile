# Build environment setup
FROM rust:1.70.0 as builder

ARG USER_ID
ARG GROUP_ID

# Create a new user with the host user's ID and group ID
# This enables mounting the project directory as a volume for build caching
RUN groupadd -g $GROUP_ID user && useradd -l -u $USER_ID -g user user
USER user

WORKDIR /build

# Install wasm32-wasi target to build for WASM
RUN rustup target add wasm32-wasi

# The /build directory will be mounted from the project repo by docker-compose
# along with the run command and build artifacts will be cached locally as well
# examples/envoy/docker-compose.yaml has an example of how to mount the build directory
# as a volume and cargo build the project