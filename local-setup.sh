#!/bin/bash

# Install infrastructure dependencies
brew install terraform kubectl azure-cli helm Azure/kubelogin/kubelogin

# authenticate to azure for terraform and kubectl
az login


# Azure Kubternetes Service (AKS) cluster creation
# ================================================

# in the terraform repo, initialize terraform
export ARM_ACCESS_KEY=$(az storage account keys list --resource-group terraform --account-name moesifterraform --query '[0].value' -o tsv)
terraform init
# terraform plan, check and apply just the aks-cluster module
# this will take around 5 minutes
terraform plan -out westus2-plan -var-file="westus2.tfvars" -target="module.aks-cluster"
terraform apply "westus2-plan"


# AKS Cluster configuration for Gloo
# ===================================

# get the kubeconfig for the cluster to authenticate kubectl
az aks get-credentials --resource-group local-development --name moesif-aks-cluster
# initalize helm with solo.io repo
helm repo add gloo https://storage.googleapis.com/solo-public-helm
helm repo update
# install gloo in the gloo-system namespace
kubectl create namespace gloo-system
helm install gloo gloo/gloo --namespace gloo-system
kubectl -n gloo-system get svc

# deploy the echo service
kubectl apply -f echo.yaml
# create a TLS certificate for the Gloo ingress
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout tls.key -out tls.crt -subj "/CN=cluster-dev.internal.moesif.com"
kubectl create secret tls gloo-tls --key tls.key --cert tls.crt -n gloo-system
# deploy the echo virtual services and routes
kubectl apply -f echo-virtualservice.yaml
# authenticate k8s to azure container registry
# This step was performed manually by authenticating the VMSS
# for the node pool with a managed identity
