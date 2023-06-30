#!/bin/bash

NAMESPACE="istio-ingress"
LABEL="istio=ingress"

NEW_POD_NAME=$(kubectl get pods -n $NAMESPACE -l $LABEL -o jsonpath='{.items[0].metadata.name}')
kubectl logs -f $NEW_POD_NAME -n $NAMESPACE | code -