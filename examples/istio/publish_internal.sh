#!/bin/bash -e

if [ "$1" == "debug" ]; then
    BUILD_VARIANT=debug
    BUILD_FLAGS=""
else
    BUILD_VARIANT=release
    BUILD_FLAGS="--release"
fi

# Get the directory of this script to make sure we can run it from anywhere
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$SCRIPT_DIR/../.."
SOURCE="$BASE_DIR/moesif-wasm"
OUTPUT="$SOURCE/target/wasm32-wasi/$BUILD_VARIANT"

# Docker image names
TAG_BUILD=moesiftest.azurecr.io/moesif-envoy-wasm-plugin-builder:latest
TAG_ARTIFACT=moesiftest.azurecr.io/moesif-envoy-wasm-plugin:latest

# Create the build environment for the plugin
docker build \
 --build-arg USER_ID=$(id -u) --build-arg GROUP_ID=$(id -g) \
 -t $TAG_BUILD \
 -f $BASE_DIR/Cargo-build.dockerfile \
 $SOURCE

# perform the build inside the container by mounting the current directory
docker run \
 -v $SOURCE:/build \
 $TAG_BUILD \
 bash -c "cargo build --target=wasm32-wasi $BUILD_FLAGS"

# package the plugin into a docker image for deployment
docker build \
 -t $TAG_ARTIFACT \
 -f $BASE_DIR/examples/istio/artifact.dockerfile \
 $OUTPUT

docker push $TAG_ARTIFACT
