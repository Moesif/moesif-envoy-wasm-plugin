#!/bin/bash

NAMESPACE="istio-ingress"
LABEL="istio=ingress"

# Get the pod name
POD_NAME=$(kubectl get pods -n $NAMESPACE -l $LABEL -o jsonpath='{.items[0].metadata.name}')

# Delete the pod
kubectl delete pod $POD_NAME -n $NAMESPACE
