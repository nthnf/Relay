#!/usr/bin/env bash
set -euo pipefail

CLUSTER_NAME=${CLUSTER_NAME:-relay}
CONTAINER_NAME="${CLUSTER_NAME}-control-plane"

if ! docker ps -a --format '{{.Names}}' | grep -qx "$CONTAINER_NAME"; then
  echo "kind control-plane container not found: ${CONTAINER_NAME}" >&2
  exit 1
fi

echo "Starting kind cluster container: ${CONTAINER_NAME}"
docker start "$CONTAINER_NAME" >/dev/null

echo "Refreshing kubeconfig for kind cluster: ${CLUSTER_NAME}"
kind export kubeconfig --name "$CLUSTER_NAME" >/dev/null

echo "Waiting for Kubernetes API"
for _ in $(seq 1 60); do
  if kubectl get --raw=/readyz >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
kubectl get --raw=/readyz >/dev/null

echo "Waiting for system pods"
kubectl wait --timeout=5m -n kube-system pods --all --for=condition=Ready >/dev/null || true

echo "Waiting for Relay workloads"
kubectl wait --timeout=5m -n relay pods --all --for=condition=Ready >/dev/null || true

if kubectl get namespace envoy-gateway-system >/dev/null 2>&1; then
  echo "Waiting for Envoy Gateway"
  kubectl wait --timeout=5m -n envoy-gateway-system deployment/envoy-gateway --for=condition=Available >/dev/null || true
fi

if kubectl get namespace metallb-system >/dev/null 2>&1; then
  echo "Waiting for MetalLB"
  kubectl wait --timeout=5m -n metallb-system deployment/metallb-controller --for=condition=Available >/dev/null || true
  kubectl rollout status daemonset/metallb-speaker -n metallb-system --timeout=5m >/dev/null || true
fi

echo "Cluster '${CLUSTER_NAME}' resumed. Current Envoy LoadBalancer service:"
kubectl get svc -n envoy-gateway-system 2>/dev/null || true
