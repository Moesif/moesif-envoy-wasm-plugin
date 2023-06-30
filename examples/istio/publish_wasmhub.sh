#!/bin/bash -e
TAG=${1:-latest}

if [ "$2" == "debug" ]; then
    BUILD_VARIANT=debug
    BUILD_FLAGS=""
else
    BUILD_VARIANT=release
    BUILD_FLAGS="--release"
fi

TAG=${1:-latest}

WASME=$HOME/.wasme/bin/wasme

# Get the directory of this script to make sure we can run it from anywhere
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$SCRIPT_DIR/../.."
SOURCE="$BASE_DIR/moesif-wasm"
OUTPUT="$SOURCE/target/wasm32-wasi/$BUILD_VARIANT"

# Docker image names
TAG_BUILD=moesiftest.azurecr.io/moesif-envoy-wasm-plugin-builder:latest
TAG_ARTIFACT=webassemblyhub.io/brian_moesif/moesif_envoy_wasm_plugin

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
VERSION_TAG=$TAG_ARTIFACT:$TAG
$WASME build precompiled moesif-wasm/target/wasm32-wasi/$BUILD_VARIANT/moesif_envoy_wasm_plugin.wasm --tag $VERSION_TAG
$WASME push $VERSION_TAG

LATEST_TAG=$TAG_ARTIFACT:latest
$WASME tag $VERSION_TAG $LATEST_TAG
$WASME push $LATEST_TAG
