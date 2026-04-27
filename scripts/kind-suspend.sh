#!/usr/bin/env bash
set -euo pipefail

CLUSTER_NAME=${CLUSTER_NAME:-relay}
CONTAINER_NAME="${CLUSTER_NAME}-control-plane"

if ! docker ps -a --format '{{.Names}}' | grep -qx "$CONTAINER_NAME"; then
  echo "kind control-plane container not found: ${CONTAINER_NAME}" >&2
  exit 1
fi

echo "Stopping kind cluster container: ${CONTAINER_NAME}"
docker stop "$CONTAINER_NAME" >/dev/null
echo "Suspended kind cluster '${CLUSTER_NAME}'. State is preserved because the container was stopped, not deleted."
