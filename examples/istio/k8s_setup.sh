#!/bin/bash


kubectl apply -f echo.yaml \
 istio-moesif-upstream.yaml \
 istio-envoy-filter.yaml \
 istio-echo-gateway-vservice.yaml
