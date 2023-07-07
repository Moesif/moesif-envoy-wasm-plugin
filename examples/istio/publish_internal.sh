#!/bin/bash -e

TAG=${1:-latest}
if [ "$2" == "debug" ]; then
    BUILD_VARIANT=debug
    BUILD_FLAGS=""
else
    BUILD_VARIANT=release
    BUILD_FLAGS="--release"
fi

# Docker image names
REPO=docker.io/brianmoesif/
TAG_BUILD=$REPO/moesif-envoy-wasm-plugin-builder:latest
TAG_ARTIFACT=$REPO/moesif-envoy-wasm-plugin:$TAG
TAG_LATEST=$REPO/moesif-envoy-wasm-plugin:latest

# Get the directory of this script to make sure we can run it from anywhere
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$SCRIPT_DIR/../.."
SOURCE="$BASE_DIR/moesif-wasm"
OUTPUT="$SOURCE/target/wasm32-wasi/$BUILD_VARIANT"

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

docker tag $TAG_ARTIFACT $TAG_LATEST
docker push $TAG_LATEST
