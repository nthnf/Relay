#!/usr/bin/env bash
set -euo pipefail

CLUSTER_NAME=${CLUSTER_NAME:-relay}

if kind get clusters | grep -qx "$CLUSTER_NAME"; then
  echo "Deleting kind cluster: ${CLUSTER_NAME}"
  kind delete cluster --name "$CLUSTER_NAME"
else
  echo "kind cluster '${CLUSTER_NAME}' does not exist; nothing to delete"
fi
