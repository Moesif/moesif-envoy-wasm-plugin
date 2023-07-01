#!/bin/bash


kubectl apply -f echo-service.yaml \
 -f istio-echo-inbound.yaml \
 -f istio-moesif-outbound.yaml \
 -f moesif-wasm-plugin.yaml
