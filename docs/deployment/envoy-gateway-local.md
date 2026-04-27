# Envoy Gateway local setup

This guide installs Envoy Gateway for the `deployment/k8s/local-kind-envoy` overlay and runs the registration-to-chat E2E through Envoy.

## Prerequisites

- Running kind cluster from `deployment/k8s/local-kind/kind-config.yaml`
- `kubectl`
- `helm`
- `grpcurl`
- `jq`
- `websocat`

## Install Envoy Gateway

```bash
helm install eg oci://docker.io/envoyproxy/gateway-helm --version 1.7.2 -n envoy-gateway-system --create-namespace
kubectl wait --timeout=5m -n envoy-gateway-system deployment/envoy-gateway --for=condition=Available
```

## Apply Relay Envoy Overlay

```bash
kubectl apply -k deployment/k8s/local-kind-envoy
kubectl -n relay wait --timeout=5m gateway/relay-gateway --for=condition=Accepted
```

The overlay creates:

- `bootstrap.local` protected gRPC route using `identity` as gRPC ext-auth.
- `identity.local` public gRPC route.
- `chat.local` protected gRPC route using `identity` as gRPC ext-auth.
- `workspace.local` protected gRPC route using `identity` as gRPC ext-auth.
- `realtime.local` protected WebSocket route for `/ws` using `identity` as gRPC ext-auth.

## Run E2E

```bash
./e2e/chat-envoy.sh
```

The script discovers the Envoy-managed data-plane Service, port-forwards it locally, registers two users through Envoy, redeems email verification tokens, creates a protected DM conversation through Envoy, connects Bob to realtime through Envoy, sends Alice's message, and verifies Bob receives `message_created`.

In kind, `Gateway/relay-gateway` can remain top-level `Programmed=False` because no LoadBalancer address is assigned. That is acceptable for this local flow; the script uses the generated Envoy Service with `kubectl port-forward`.

## Optional MetalLB LoadBalancer

To test with a real local `EXTERNAL-IP` on the Envoy `LoadBalancer` Service, install MetalLB:

```bash
helm repo add metallb https://metallb.github.io/metallb
helm repo update metallb
helm install metallb metallb/metallb --version 0.15.3 -n metallb-system --create-namespace
kubectl wait --timeout=5m -n metallb-system deployment/metallb-controller --for=condition=Available
kubectl wait --timeout=5m -n metallb-system daemonset/metallb-speaker --for=jsonpath='{.status.numberReady}'=1
kubectl apply -k deployment/k8s/local-kind-metallb
```

The current pool is `172.19.255.200-172.19.255.250`, matching this kind Docker network. After MetalLB assigns an address, `./e2e/chat-envoy.sh` uses it automatically.

## Manual Port Forward

If you need manual access, discover the Envoy Service:

```bash
export ENVOY_SERVICE=$(kubectl get svc -n envoy-gateway-system \
  --selector=gateway.envoyproxy.io/owning-gateway-namespace=relay,gateway.envoyproxy.io/owning-gateway-name=relay-gateway \
  -o jsonpath='{.items[0].metadata.name}')
```

Then forward both listeners:

```bash
kubectl -n envoy-gateway-system port-forward service/${ENVOY_SERVICE} 18080:8080 18081:8081
```

Use gRPC authority headers for service routing, for example `identity.local` or `chat.local`.
