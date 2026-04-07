# Envoy Gateway local setup

This guide installs Envoy Gateway into a local Kubernetes cluster, applies the repo-managed Gateway API manifests, port-forwards the managed Envoy data plane, and verifies the smoke-test gRPC route with `grpcurl`.

## Prerequisites

- A running local Kubernetes cluster, such as `kind`
- `kubectl`
- `helm`
- `grpcurl`

## 1. Install Envoy Gateway

Install Envoy Gateway with Helm in `envoy-gateway-system`:

```bash
helm install eg oci://docker.io/envoyproxy/gateway-helm --version v1.7.1 -n envoy-gateway-system --create-namespace
```

Wait for the controller deployment to become available:

```bash
kubectl wait --timeout=5m -n envoy-gateway-system deployment/envoy-gateway --for=condition=Available
```

## 2. Apply the repo manifests

Create the namespace first:

```bash
kubectl apply -f deployment/envoy-gateway/namespace.yaml
```

Then apply the remaining manifests in a safe order:

```bash
kubectl apply -f deployment/envoy-gateway/gateway.yaml \
  -f deployment/envoy-gateway/smoke-test-grpc-backend.yaml \
  -f deployment/envoy-gateway/grpcroute-identity.yaml \
  -f deployment/envoy-gateway/security-policy-placeholder.yaml \
  -f deployment/envoy-gateway/backend-traffic-policy-placeholder.yaml
```

Wait for the smoke-test backend to become available:

```bash
kubectl wait --timeout=5m -n relay-system deployment/grpc-smoke --for=condition=Available
```

Wait for the Envoy-managed data-plane Service to exist:

```bash
kubectl wait --timeout=5m -n envoy-gateway-system \
  --for=jsonpath='{.items[0].metadata.name}' \
  service -l gateway.envoyproxy.io/owning-gateway-namespace=relay-system,gateway.envoyproxy.io/owning-gateway-name=relay-gateway
```

## 3. Discover the managed Envoy Service

Envoy Gateway creates the data-plane Service for `relay-system/relay-gateway`. Discover it with the ownership labels:

```bash
export ENVOY_SERVICE=$(kubectl get svc -n envoy-gateway-system \
  --selector=gateway.envoyproxy.io/owning-gateway-namespace=relay-system,gateway.envoyproxy.io/owning-gateway-name=relay-gateway \
  -o jsonpath='{.items[0].metadata.name}')
```

## 4. Port-forward locally

Forward the managed Envoy Service to `127.0.0.1:8080`:

```bash
kubectl -n envoy-gateway-system port-forward service/${ENVOY_SERVICE} 8080:8080
```

Keep that terminal running.

## 5. Verify the smoke-test gRPC route

In a second terminal, verify the smoke-test route with `grpcurl`:

```bash
grpcurl -plaintext 127.0.0.1:8080 list
```

The smoke-test route does not require an authority override. A successful result lists reflected services from the `grpc-smoke` backend, including `yages.Echo`.

## 6. Identity route status

The repo also includes an `identity` `GRPCRoute` with hostname `identity.local`. It is structurally ready, but it will not pass traffic until a real Kubernetes `Service` named `identity` exists in `relay-system` and serves gRPC on port `50051`.

Once that Service exists, the route can become usable through Envoy Gateway. Because the route still matches `identity.local`, local clients will need to send that hostname/authority when testing it.

## 7. Placeholder policy files

`deployment/envoy-gateway/security-policy-placeholder.yaml` and `deployment/envoy-gateway/backend-traffic-policy-placeholder.yaml` are placeholders only. They intentionally do not define the final auth, retry, timeout, or traffic-policy behavior for Relay yet.
