#!/usr/bin/env bash
set -euo pipefail

NAMESPACE=${NAMESPACE:-relay}
ENVOY_NAMESPACE=${ENVOY_NAMESPACE:-envoy-gateway-system}
GATEWAY_NAME=${GATEWAY_NAME:-relay-gateway}
GATEWAY_GRPC_LOCAL_PORT=${GATEWAY_GRPC_LOCAL_PORT:-18080}
GATEWAY_HTTP_LOCAL_PORT=${GATEWAY_HTTP_LOCAL_PORT:-18081}
APPLY_MANIFESTS=${APPLY_MANIFESTS:-1}
USE_LOADBALANCER=${USE_LOADBALANCER:-auto}
GRPCURL_TIMEOUT=${GRPCURL_TIMEOUT:-20s}
KUBECTL_TIMEOUT=${KUBECTL_TIMEOUT:-60s}

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
TMP_DIR=$(mktemp -d)

cleanup() {
  if [[ -n "${ENVOY_PF_PID:-}" ]]; then kill "$ENVOY_PF_PID" 2>/dev/null || true; fi
  if [[ -n "${WS_PID:-}" ]]; then kill "$WS_PID" 2>/dev/null || true; fi
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

log() {
  printf '[e2e] %s\n' "$*" >&2
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

wait_tcp() {
  local port=$1
  for _ in $(seq 1 60); do
    if (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; then
      exec 3>&-
      return 0
    fi
    sleep 0.25
  done
  echo "port $port did not open" >&2
  return 1
}

grpc_identity() {
  local args=("$@")
  local method_index=$((${#args[@]} - 1))
  local method=${args[$method_index]}
  unset 'args[$method_index]'
  timeout "$GRPCURL_TIMEOUT" grpcurl -plaintext \
    -authority identity.local \
    -import-path "$ROOT_DIR/proto" \
    -proto identity.proto \
    "${args[@]}" \
    "${GATEWAY_GRPC_ADDR}" \
    "$method"
}

grpc_chat() {
  local args=("$@")
  local method_index=$((${#args[@]} - 1))
  local method=${args[$method_index]}
  unset 'args[$method_index]'
  timeout "$GRPCURL_TIMEOUT" grpcurl -plaintext \
    -authority chat.local \
    -import-path "$ROOT_DIR/proto" \
    -proto chat.proto \
    "${args[@]}" \
    "${GATEWAY_GRPC_ADDR}" \
    "$method"
}

verification_token() {
  local user_id=$1
  timeout "$KUBECTL_TIMEOUT" kubectl -n "$NAMESPACE" exec identity-postgres-0 -- \
    psql -U relay -d relay -tAc \
      "select payload->>'verification_token' from outbox_event where aggregate_id = '$user_id' and event_type = 'VerificationEmailRequested' order by created_at desc limit 1;" \
    | tr -d '[:space:]'
}

wait_gateway_crds() {
  kubectl get crd gateways.gateway.networking.k8s.io >/dev/null
  kubectl get crd grpcroutes.gateway.networking.k8s.io >/dev/null
  kubectl get crd httproutes.gateway.networking.k8s.io >/dev/null
  kubectl get crd securitypolicies.gateway.envoyproxy.io >/dev/null
}

require_cmd kubectl
require_cmd grpcurl
require_cmd jq
require_cmd websocat
require_cmd timeout

log "checking Envoy Gateway CRDs"
if ! wait_gateway_crds; then
  echo "Envoy Gateway/Gateway API CRDs are missing. Install Envoy Gateway first, then rerun." >&2
  echo "Expected install command: helm install eg oci://docker.io/envoyproxy/gateway-helm --version 1.7.2 -n envoy-gateway-system --create-namespace" >&2
  exit 1
fi

if [[ "$APPLY_MANIFESTS" == "1" ]]; then
  log "applying deployment/k8s/local-kind-envoy"
  timeout "$KUBECTL_TIMEOUT" kubectl apply -k "$ROOT_DIR/deployment/k8s/local-kind-envoy" >/dev/null
fi

log "waiting for gateway/${GATEWAY_NAME} Accepted=True"
if ! kubectl -n "$NAMESPACE" wait --timeout=2m gateway/"$GATEWAY_NAME" --for=condition=Accepted >/dev/null; then
  log "gateway did not become Accepted; current status follows"
  kubectl -n "$NAMESPACE" describe gateway/"$GATEWAY_NAME" >&2 || true
  kubectl -n "$NAMESPACE" get gateway,grpcroute,httproute,securitypolicy >&2 || true
  exit 1
fi

log "discovering Envoy data-plane service"
ENVOY_SERVICE=""
for _ in $(seq 1 60); do
  ENVOY_SERVICE=$(kubectl get svc -n "$ENVOY_NAMESPACE" \
    --selector="gateway.envoyproxy.io/owning-gateway-namespace=${NAMESPACE},gateway.envoyproxy.io/owning-gateway-name=${GATEWAY_NAME}" \
    -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || true)
  if [[ -n "$ENVOY_SERVICE" ]]; then
    break
  fi
  sleep 1
done

if [[ -z "$ENVOY_SERVICE" ]]; then
  echo "could not discover Envoy data-plane service for ${NAMESPACE}/${GATEWAY_NAME}" >&2
  exit 1
fi

ENVOY_EXTERNAL_IP=$(kubectl get svc -n "$ENVOY_NAMESPACE" "$ENVOY_SERVICE" -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null || true)
if [[ "$USE_LOADBALANCER" != "never" && -n "$ENVOY_EXTERNAL_IP" ]]; then
  GATEWAY_GRPC_ADDR="${ENVOY_EXTERNAL_IP}:8080"
  GATEWAY_HTTP_WS_ADDR="${ENVOY_EXTERNAL_IP}:8081"
  log "using Envoy LoadBalancer external IP ${ENVOY_EXTERNAL_IP}"
else
  if [[ "$USE_LOADBALANCER" == "always" ]]; then
    echo "Envoy LoadBalancer external IP is not assigned" >&2
    exit 1
  fi
  GATEWAY_GRPC_ADDR="127.0.0.1:${GATEWAY_GRPC_LOCAL_PORT}"
  GATEWAY_HTTP_WS_ADDR="127.0.0.1:${GATEWAY_HTTP_LOCAL_PORT}"
  log "port-forwarding Envoy service/${ENVOY_SERVICE} to ${GATEWAY_GRPC_LOCAL_PORT}:8080 and ${GATEWAY_HTTP_LOCAL_PORT}:8081"
  kubectl -n "$ENVOY_NAMESPACE" port-forward "service/${ENVOY_SERVICE}" \
    "${GATEWAY_GRPC_LOCAL_PORT}:8080" \
    "${GATEWAY_HTTP_LOCAL_PORT}:8081" \
    >"$TMP_DIR/envoy-port-forward.log" 2>&1 &
  ENVOY_PF_PID=$!
  wait_tcp "$GATEWAY_GRPC_LOCAL_PORT"
  wait_tcp "$GATEWAY_HTTP_LOCAL_PORT"
  log "Envoy port-forward is ready"
fi

RUN_ID=$(date +%s%N)
ALICE_EMAIL="alice-${RUN_ID}@example.test"
BOB_EMAIL="bob-${RUN_ID}@example.test"

log "registering Alice through identity.local"
ALICE_JSON=$(grpc_identity \
  -d "{\"email\":\"${ALICE_EMAIL}\",\"password\":\"Password123!\",\"username\":\"alice${RUN_ID}\",\"displayName\":\"Alice E2E\"}" \
  relay.identity.IdentityService/RegisterUser)
log "registering Bob through identity.local"
BOB_JSON=$(grpc_identity \
  -d "{\"email\":\"${BOB_EMAIL}\",\"password\":\"Password123!\",\"username\":\"bob${RUN_ID}\",\"displayName\":\"Bob E2E\"}" \
  relay.identity.IdentityService/RegisterUser)

ALICE_ID=$(jq -r .userId <<<"$ALICE_JSON")
BOB_ID=$(jq -r .userId <<<"$BOB_JSON")

if [[ -z "$ALICE_ID" || "$ALICE_ID" == "null" || -z "$BOB_ID" || "$BOB_ID" == "null" ]]; then
  echo "registration did not return user ids" >&2
  exit 1
fi

log "reading local verification tokens from identity outbox"
ALICE_VERIFY_TOKEN=$(verification_token "$ALICE_ID")
BOB_VERIFY_TOKEN=$(verification_token "$BOB_ID")

if [[ -z "$ALICE_VERIFY_TOKEN" || -z "$BOB_VERIFY_TOKEN" ]]; then
  echo "could not read verification tokens from identity outbox" >&2
  exit 1
fi

log "redeeming Alice verification token through identity.local"
ALICE_AUTH_JSON=$(grpc_identity \
  -d "{\"token\":\"${ALICE_VERIFY_TOKEN}\"}" \
  relay.identity.IdentityService/RedeemEmailVerificationToken)
log "redeeming Bob verification token through identity.local"
BOB_AUTH_JSON=$(grpc_identity \
  -d "{\"token\":\"${BOB_VERIFY_TOKEN}\"}" \
  relay.identity.IdentityService/RedeemEmailVerificationToken)

ALICE_ACCESS_TOKEN=$(jq -r .accessToken <<<"$ALICE_AUTH_JSON")
BOB_ACCESS_TOKEN=$(jq -r .accessToken <<<"$BOB_AUTH_JSON")

if [[ -z "$ALICE_ACCESS_TOKEN" || "$ALICE_ACCESS_TOKEN" == "null" || -z "$BOB_ACCESS_TOKEN" || "$BOB_ACCESS_TOKEN" == "null" ]]; then
  echo "token redemption did not return access tokens" >&2
  exit 1
fi

CONV_JSON=""
log "creating protected DM conversation through chat.local"
for attempt in $(seq 1 30); do
  set +e
  CONV_JSON=$(grpc_chat \
    -H "authorization: Bearer ${ALICE_ACCESS_TOKEN}" \
    -d "{\"targetType\":\"CONVERSATION_TARGET_TYPE_DM\",\"peerUserId\":\"${BOB_ID}\"}" \
    relay.chat.ChatService/CreateConversation \
    2>"$TMP_DIR/create-conversation.err")
  status=$?
  set -e
  if [[ $status -eq 0 ]]; then
    break
  fi
  if [[ $attempt -eq 30 ]]; then
    cat "$TMP_DIR/create-conversation.err" >&2
    exit 1
  fi
  log "conversation not ready yet, retry ${attempt}/30"
  sleep 1
done

CONVERSATION_ID=$(jq -r .conversationId <<<"$CONV_JSON")
if [[ -z "$CONVERSATION_ID" || "$CONVERSATION_ID" == "null" ]]; then
  echo "conversation creation did not return conversation id" >&2
  exit 1
fi

WS_OUT="$TMP_DIR/ws.out"
WS_IN="$TMP_DIR/ws.in"
mkfifo "$WS_IN"
log "opening protected realtime WebSocket through realtime.local"
websocat \
  -t \
  --ws-c-uri=ws://realtime.local/ws \
  -H="authorization: Bearer ${BOB_ACCESS_TOKEN}" \
  "ws-c:tcp:${GATEWAY_HTTP_WS_ADDR}" \
  - \
  <"$WS_IN" >"$WS_OUT" 2>"$TMP_DIR/ws.err" &
WS_PID=$!
sleep 0.5
if ! kill -0 "$WS_PID" 2>/dev/null; then
  log "websocat exited before subscription; stderr follows"
  cat "$TMP_DIR/ws.err" >&2 || true
  exit 1
fi
exec 9<>"$WS_IN"

log "subscribing Bob to conversation ${CONVERSATION_ID}"
printf '{"type":"subscribe","target_kind":"direct_message","target_id":"%s"}\n' "$CONVERSATION_ID" >&9
sleep 0.5

log "sending Alice message through chat.local"
MSG_JSON=$(grpc_chat \
  -H "authorization: Bearer ${ALICE_ACCESS_TOKEN}" \
  -d "{\"conversationId\":\"${CONVERSATION_ID}\",\"body\":\"hello bob from envoy e2e ${RUN_ID}\",\"clientMessageId\":\"client-${RUN_ID}\"}" \
  relay.chat.ChatService/CreateMessage)
MESSAGE_ID=$(jq -r .messageId <<<"$MSG_JSON")

if [[ -z "$MESSAGE_ID" || "$MESSAGE_ID" == "null" ]]; then
  echo "message creation did not return message id" >&2
  exit 1
fi

log "waiting for Bob to receive realtime message ${MESSAGE_ID}"
for _ in $(seq 1 60); do
  if [[ -s "$WS_OUT" ]] && jq -e --arg mid "$MESSAGE_ID" \
    'select(.payload.type == "message_created" and .payload.message_id == $mid)' \
    "$WS_OUT" >/dev/null 2>&1; then
    RECEIVED=$(jq -c --arg mid "$MESSAGE_ID" \
      'select(.payload.type == "message_created" and .payload.message_id == $mid)' \
      "$WS_OUT")
    printf 'E2E_PASS=1\n'
    printf 'envoy_service=%s\n' "$ENVOY_SERVICE"
    printf 'envoy_external_ip=%s\n' "${ENVOY_EXTERNAL_IP:-}"
    printf 'alice_user_id=%s\n' "$ALICE_ID"
    printf 'bob_user_id=%s\n' "$BOB_ID"
    printf 'conversation_id=%s\n' "$CONVERSATION_ID"
    printf 'message_id=%s\n' "$MESSAGE_ID"
    printf 'received=%s\n' "$RECEIVED"
    exit 0
  fi
  sleep 0.25
done

printf 'E2E_PASS=0\n' >&2
printf 'alice_user_id=%s\n' "$ALICE_ID" >&2
printf 'bob_user_id=%s\n' "$BOB_ID" >&2
printf 'conversation_id=%s\n' "$CONVERSATION_ID" >&2
printf 'message_id=%s\n' "$MESSAGE_ID" >&2
printf 'websocket_stdout=\n' >&2
cat "$WS_OUT" >&2 || true
printf 'websocket_stderr=\n' >&2
cat "$TMP_DIR/ws.err" >&2 || true
exit 1
