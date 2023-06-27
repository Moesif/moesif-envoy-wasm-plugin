#!/bin/bash

helm repo add istio https://istio-release.storage.googleapis.com/charts
helm repo update

helm install istio-base istio/base -n istio-system --set defaultRevision=default --create-namespace
helm install istiod istio/istiod -n istio-system --wait

helm upgrade istio-ingress istio/gateway -n istio-ingress \              
  --set podAnnotations."sidecar\.istio\.io/logLevel"=info \
  --set podAnnotations."sidecar\.istio\.io/componentLogLevel"="wasm:trace" \
  --wait \
  --debug