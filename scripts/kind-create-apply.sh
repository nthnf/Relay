#!/usr/bin/env bash
set -euo pipefail

CLUSTER_NAME=${CLUSTER_NAME:-relay}
ENVOY_GATEWAY_VERSION=${ENVOY_GATEWAY_VERSION:-1.7.2}
METALLB_VERSION=${METALLB_VERSION:-0.15.3}
SKIP_BUILD=${SKIP_BUILD:-0}
SKIP_ENVOY=${SKIP_ENVOY:-0}
SKIP_METALLB=${SKIP_METALLB:-0}

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

IMAGES=(
  relay/identity:local
  relay/bootstrap:local
  relay/chat:local
  relay/email:local
  relay/friendship:local
  relay/workspace:local
  relay/realtime:local
  relay/outbox:local
)

log() {
  printf '[kind-create-apply] %s\n' "$*" >&2
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

build_images() {
  log "building service images"
  docker build -f "$ROOT_DIR/services/identity/Dockerfile" -t relay/identity:local "$ROOT_DIR"
  docker build -f "$ROOT_DIR/services/bootstrap/Dockerfile" -t relay/bootstrap:local "$ROOT_DIR"
  docker build -f "$ROOT_DIR/services/chat/Dockerfile" -t relay/chat:local "$ROOT_DIR"
  docker build -f "$ROOT_DIR/services/email/Dockerfile" -t relay/email:local "$ROOT_DIR"
  docker build -f "$ROOT_DIR/services/friendship/Dockerfile" -t relay/friendship:local "$ROOT_DIR"
  docker build -f "$ROOT_DIR/services/workspace/Dockerfile" -t relay/workspace:local "$ROOT_DIR"
  docker build -f "$ROOT_DIR/services/realtime/Dockerfile" -t relay/realtime:local "$ROOT_DIR"
  docker build -f "$ROOT_DIR/workers/outbox/Dockerfile" -t relay/outbox:local "$ROOT_DIR"
}

wait_for_relay() {
  log "waiting for Relay migrations"
  kubectl wait --timeout=5m -n relay --for=condition=Complete job/identity-migration
  kubectl wait --timeout=5m -n relay --for=condition=Complete job/bootstrap-migration
  kubectl wait --timeout=5m -n relay --for=condition=Complete job/chat-migration
  kubectl wait --timeout=5m -n relay --for=condition=Complete job/email-migration
  kubectl wait --timeout=5m -n relay --for=condition=Complete job/friendship-migration
  kubectl wait --timeout=5m -n relay --for=condition=Complete job/workspace-migration

  log "restarting app deployments after migrations"
  kubectl rollout restart \
    deployment/bootstrap \
    deployment/chat \
    deployment/chat-outbox \
    deployment/email \
    deployment/friendship \
    deployment/friendship-outbox \
    deployment/identity \
    deployment/identity-outbox \
    deployment/realtime \
    deployment/workspace \
    deployment/workspace-outbox \
    -n relay >/dev/null

  log "waiting for Relay deployments"
  kubectl rollout status deployment/bootstrap -n relay --timeout=5m
  kubectl rollout status deployment/chat -n relay --timeout=5m
  kubectl rollout status deployment/chat-outbox -n relay --timeout=5m
  kubectl rollout status deployment/email -n relay --timeout=5m
  kubectl rollout status deployment/friendship -n relay --timeout=5m
  kubectl rollout status deployment/friendship-outbox -n relay --timeout=5m
  kubectl rollout status deployment/identity -n relay --timeout=5m
  kubectl rollout status deployment/identity-outbox -n relay --timeout=5m
  kubectl rollout status deployment/realtime -n relay --timeout=5m
  kubectl rollout status deployment/workspace -n relay --timeout=5m
  kubectl rollout status deployment/workspace-outbox -n relay --timeout=5m
}

require_cmd docker
require_cmd kind
require_cmd kubectl
require_cmd helm

if kind get clusters | grep -qx "$CLUSTER_NAME"; then
  echo "kind cluster '${CLUSTER_NAME}' already exists. Delete it first with ./scripts/kind-delete.sh or use suspend/resume." >&2
  exit 1
fi

if [[ "$SKIP_BUILD" != "1" ]]; then
  build_images
else
  log "skipping image build because SKIP_BUILD=1"
fi

log "creating kind cluster: ${CLUSTER_NAME}"
kind create cluster --name "$CLUSTER_NAME" --config "$ROOT_DIR/deployment/k8s/local-kind/kind-config.yaml"

log "loading images into kind cluster"
kind load docker-image --name "$CLUSTER_NAME" "${IMAGES[@]}"

if [[ "$SKIP_ENVOY" != "1" ]]; then
  log "installing Envoy Gateway ${ENVOY_GATEWAY_VERSION}"
  helm install eg oci://docker.io/envoyproxy/gateway-helm \
    --version "$ENVOY_GATEWAY_VERSION" \
    -n envoy-gateway-system \
    --create-namespace
  kubectl wait --timeout=5m -n envoy-gateway-system deployment/envoy-gateway --for=condition=Available
else
  log "skipping Envoy Gateway install because SKIP_ENVOY=1"
fi

if [[ "$SKIP_METALLB" != "1" ]]; then
  log "installing MetalLB ${METALLB_VERSION}"
  helm repo add metallb https://metallb.github.io/metallb >/dev/null 2>&1 || true
  helm repo update metallb
  helm install metallb metallb/metallb \
    --version "$METALLB_VERSION" \
    -n metallb-system \
    --create-namespace
  kubectl wait --timeout=5m -n metallb-system deployment/metallb-controller --for=condition=Available
  kubectl rollout status daemonset/metallb-speaker -n metallb-system --timeout=5m
  kubectl apply -k "$ROOT_DIR/deployment/k8s/local-kind-metallb"
else
  log "skipping MetalLB install because SKIP_METALLB=1"
fi

log "applying Relay Kubernetes overlay"
if [[ "$SKIP_ENVOY" == "1" ]]; then
  kubectl apply -k "$ROOT_DIR/deployment/k8s/local-kind"
else
  kubectl apply -k "$ROOT_DIR/deployment/k8s/local-kind-envoy"
fi

wait_for_relay

if [[ "$SKIP_ENVOY" != "1" ]]; then
  log "waiting for Gateway acceptance"
  kubectl wait --timeout=5m -n relay gateway/relay-gateway --for=condition=Accepted
fi

log "cluster ready"
kubectl get pods -n relay
if [[ "$SKIP_ENVOY" != "1" ]]; then
  kubectl get gateway -n relay
  kubectl get svc -n envoy-gateway-system
fi
