#!/bin/bash


kubectl apply -f echo.yaml \
 -f istio-moesif-upstream.yaml \
 -f istio-envoy-filter.yaml \
 -f istio-echo-gateway-vservice.yaml
