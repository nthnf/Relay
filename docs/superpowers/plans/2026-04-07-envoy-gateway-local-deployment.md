# Envoy Gateway Local Deployment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add repo-managed Envoy Gateway Gateway API manifests plus local deployment docs so a developer can install Envoy Gateway on `kind`, apply the project resources, port-forward the managed Envoy service, and verify a working gRPC route immediately.

**Architecture:** Envoy Gateway is installed with Helm as the backend ingress controller. The repo owns the `Gateway`, `GRPCRoute`, namespace, smoke-test backend, and policy placeholder manifests under `deployment/envoy-gateway/`, while `docs/deployment/envoy-gateway-local.md` documents the local install, apply, port-forward, and `grpcurl` verification flow.

**Tech Stack:** Kubernetes Gateway API YAML, Envoy Gateway Helm install, GRPCRoute, Deployment/Service manifests, Markdown deployment docs, `kubectl`, `helm`, `grpcurl`

---

## File Structure

- Create: `deployment/envoy-gateway/namespace.yaml`
- Create: `deployment/envoy-gateway/gateway.yaml`
- Create: `deployment/envoy-gateway/grpcroute-identity.yaml`
- Create: `deployment/envoy-gateway/security-policy-placeholder.yaml`
- Create: `deployment/envoy-gateway/backend-traffic-policy-placeholder.yaml`
- Create: `deployment/envoy-gateway/smoke-test-grpc-backend.yaml`
- Create: `docs/deployment/envoy-gateway-local.md`

### Task 1: Add Repo-Managed Envoy Gateway Resources

**Files:**
- Create: `deployment/envoy-gateway/namespace.yaml`
- Create: `deployment/envoy-gateway/gateway.yaml`
- Create: `deployment/envoy-gateway/grpcroute-identity.yaml`
- Create: `deployment/envoy-gateway/security-policy-placeholder.yaml`
- Create: `deployment/envoy-gateway/backend-traffic-policy-placeholder.yaml`
- Create: `deployment/envoy-gateway/smoke-test-grpc-backend.yaml`

- [ ] **Step 1: Create the backend namespace manifest**

Write `deployment/envoy-gateway/namespace.yaml` with this exact content:

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: relay-system
  labels:
    app.kubernetes.io/name: relay-system
    app.kubernetes.io/part-of: relay
```

- [ ] **Step 2: Create the GatewayClass and Gateway manifest**

Write `deployment/envoy-gateway/gateway.yaml` with this exact content:

```yaml
apiVersion: gateway.networking.k8s.io/v1
kind: GatewayClass
metadata:
  name: relay-envoy-local
  labels:
    app.kubernetes.io/name: relay-envoy-local
    app.kubernetes.io/part-of: relay
spec:
  controllerName: gateway.envoyproxy.io/gatewayclass-controller
---
apiVersion: gateway.networking.k8s.io/v1
kind: Gateway
metadata:
  name: relay-gateway
  namespace: relay-system
  labels:
    app.kubernetes.io/name: relay-gateway
    app.kubernetes.io/part-of: relay
spec:
  gatewayClassName: relay-envoy-local
  listeners:
    - name: grpc
      protocol: HTTP
      port: 8080
      allowedRoutes:
        namespaces:
          from: Same
```

Reasoning captured by the resource itself:
- local development uses cleartext HTTP/2
- routing stays namespace-local for now
- no TLS or broad cross-namespace route sharing in the first usable pass

- [ ] **Step 3: Create the real project GRPCRoute shape for identity**

Write `deployment/envoy-gateway/grpcroute-identity.yaml` with this exact content:

```yaml
apiVersion: gateway.networking.k8s.io/v1
kind: GRPCRoute
metadata:
  name: identity
  namespace: relay-system
  labels:
    app.kubernetes.io/name: identity
    app.kubernetes.io/part-of: relay
spec:
  parentRefs:
    - name: relay-gateway
  hostnames:
    - identity.local
  rules:
    - backendRefs:
        - group: ""
          kind: Service
          name: identity
          port: 50051
          weight: 1
```

This codifies the intended backend exposure shape even before the real `identity` Service is deployable.

- [ ] **Step 4: Create the smoke-test gRPC backend and route**

Write `deployment/envoy-gateway/smoke-test-grpc-backend.yaml` with this exact content, using the same yages image and route pattern Envoy Gateway documents for gRPC routing:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: grpc-smoke
  namespace: relay-system
  labels:
    app.kubernetes.io/name: grpc-smoke
    app.kubernetes.io/part-of: relay
spec:
  replicas: 1
  selector:
    matchLabels:
      app.kubernetes.io/name: grpc-smoke
  template:
    metadata:
      labels:
        app.kubernetes.io/name: grpc-smoke
        app.kubernetes.io/part-of: relay
    spec:
      containers:
        - name: yages
          image: ghcr.io/projectcontour/yages:v0.1.0
          imagePullPolicy: IfNotPresent
          ports:
            - containerPort: 9000
              protocol: TCP
---
apiVersion: v1
kind: Service
metadata:
  name: grpc-smoke
  namespace: relay-system
  labels:
    app.kubernetes.io/name: grpc-smoke
    app.kubernetes.io/part-of: relay
spec:
  type: ClusterIP
  selector:
    app.kubernetes.io/name: grpc-smoke
  ports:
    - name: grpc
      port: 9000
      protocol: TCP
      targetPort: 9000
---
apiVersion: gateway.networking.k8s.io/v1
kind: GRPCRoute
metadata:
  name: grpc-smoke
  namespace: relay-system
  labels:
    app.kubernetes.io/name: grpc-smoke
    app.kubernetes.io/part-of: relay
spec:
  parentRefs:
    - name: relay-gateway
  hostnames:
    - grpc-smoke.local
  rules:
    - matches:
        - method:
            service: grpc.reflection.v1alpha.ServerReflection
            method: ServerReflectionInfo
        - method:
            service: yages.Echo
            method: Ping
      backendRefs:
        - group: ""
          kind: Service
          name: grpc-smoke
          port: 9000
          weight: 1
```

- [ ] **Step 5: Create the security policy placeholder manifest as a valid, non-enforcing reference object**

Write `deployment/envoy-gateway/security-policy-placeholder.yaml` as a `ConfigMap` so the file is applyable but clearly not the final auth policy:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: envoy-gateway-security-policy-placeholder
  namespace: relay-system
  labels:
    app.kubernetes.io/name: envoy-gateway-security-policy-placeholder
    app.kubernetes.io/part-of: relay
data:
  README.md: |
    This placeholder marks where Envoy Gateway ingress auth or ext-auth policy will attach later.

    Intended attachment point:
    - Gateway: relay-gateway
    - or a specific GRPCRoute such as identity

    This file is intentionally non-enforcing in the first usable local pass because the final
    access-token and ext-auth model is still under design.

  security-policy.example.yaml: |
    apiVersion: gateway.envoyproxy.io/v1alpha1
    kind: SecurityPolicy
    metadata:
      name: identity-auth-example
      namespace: relay-system
    spec:
      targetRef:
        group: gateway.networking.k8s.io
        kind: GRPCRoute
        name: identity
      # Add jwt or extAuth configuration here once the auth model is finalized.
```

- [ ] **Step 6: Create the backend traffic policy placeholder manifest as a valid reference object**

Write `deployment/envoy-gateway/backend-traffic-policy-placeholder.yaml` as another `ConfigMap`:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: envoy-gateway-backend-traffic-policy-placeholder
  namespace: relay-system
  labels:
    app.kubernetes.io/name: envoy-gateway-backend-traffic-policy-placeholder
    app.kubernetes.io/part-of: relay
data:
  README.md: |
    This placeholder marks where Envoy Gateway backend traffic policy will attach later.

    Intended future uses:
    - request timeout policy on GRPCRoutes
    - retry policy where safe
    - rate limiting or circuit breaking when the final edge model is settled

  backend-traffic-policy.example.yaml: |
    apiVersion: gateway.envoyproxy.io/v1alpha1
    kind: BackendTrafficPolicy
    metadata:
      name: grpc-smoke-timeout-example
      namespace: relay-system
    spec:
      targetRefs:
        - group: gateway.networking.k8s.io
          kind: GRPCRoute
          name: grpc-smoke
      timeout:
        http:
          requestTimeout: 0s
```

- [ ] **Step 7: Verify the deployment manifests exist and contain the expected resource kinds**

Run: `rg -n "^kind: (Namespace|GatewayClass|Gateway|GRPCRoute|Deployment|Service|ConfigMap)" deployment/envoy-gateway`

Expected:
- namespace manifest present
- `GatewayClass` and `Gateway` present
- one identity `GRPCRoute` present
- one smoke-test `Deployment`, `Service`, and `GRPCRoute` present
- two placeholder `ConfigMap` manifests present

### Task 2: Add Local Deployment Documentation

**Files:**
- Create: `docs/deployment/envoy-gateway-local.md`

- [ ] **Step 1: Create the local deployment guide with exact install and apply commands**

Write `docs/deployment/envoy-gateway-local.md` with content equivalent to the following, using the exact commands below:

```md
# Envoy Gateway Local Deployment

## Purpose

This guide installs Envoy Gateway into a local `kind` cluster, applies the repo-managed Gateway API manifests, port-forwards the managed Envoy service, and verifies a working gRPC route with `grpcurl`.

## Prerequisites

- a running `kind` cluster
- `kubectl`
- `helm`
- `grpcurl`

## Install Envoy Gateway

```bash
helm install eg oci://docker.io/envoyproxy/gateway-helm --version v1.7.1 -n envoy-gateway-system --create-namespace
kubectl wait --timeout=5m -n envoy-gateway-system deployment/envoy-gateway --for=condition=Available
```

## Apply Repo Manifests

Apply only the manifests that are part of the usable first pass:

```bash
kubectl apply -f deployment/envoy-gateway/namespace.yaml
kubectl apply -f deployment/envoy-gateway/gateway.yaml
kubectl apply -f deployment/envoy-gateway/smoke-test-grpc-backend.yaml
kubectl apply -f deployment/envoy-gateway/grpcroute-identity.yaml
kubectl apply -f deployment/envoy-gateway/security-policy-placeholder.yaml
kubectl apply -f deployment/envoy-gateway/backend-traffic-policy-placeholder.yaml
```

## Find The Managed Envoy Service

Envoy Gateway creates a data-plane Service for the `relay-gateway` resource. Discover it with the Gateway ownership labels:

```bash
export ENVOY_NS=$(kubectl get svc -A --selector=gateway.envoyproxy.io/owning-gateway-namespace=relay-system,gateway.envoyproxy.io/owning-gateway-name=relay-gateway -o jsonpath='{.items[0].metadata.namespace}')
export ENVOY_SERVICE=$(kubectl get svc -A --selector=gateway.envoyproxy.io/owning-gateway-namespace=relay-system,gateway.envoyproxy.io/owning-gateway-name=relay-gateway -o jsonpath='{.items[0].metadata.name}')
printf "%s/%s\n" "$ENVOY_NS" "$ENVOY_SERVICE"
```

## Port-Forward Envoy Gateway Locally

```bash
kubectl -n "$ENVOY_NS" port-forward service/"$ENVOY_SERVICE" 8080:8080
```

Keep that terminal open.

## Verify The Smoke-Test gRPC Route

In another terminal:

```bash
grpcurl -plaintext -authority=grpc-smoke.local 127.0.0.1:8080 yages.Echo/Ping
```

Expected response:

```json
{
  "text": "pong"
}
```

## Identity Route Status

The repo also defines an `identity.local` `GRPCRoute` that points at the future `identity` Service:

```bash
kubectl get grpcroute -n relay-system identity -o yaml
```

That route is structurally ready now, but it becomes callable only after a real `identity` Kubernetes `Service` exists on port `50051`.

## Placeholders

- `security-policy-placeholder.yaml` documents where ingress auth or ext-auth policy will attach later.
- `backend-traffic-policy-placeholder.yaml` documents where timeout/retry/traffic policy will attach later.

These placeholders are intentionally non-enforcing in the first usable local pass.
```

- [ ] **Step 2: Verify the deployment guide includes the required local flow**

Run: `rg -n "helm install eg|kubectl wait|kubectl apply -f deployment/envoy-gateway|port-forward|grpcurl -plaintext -authority=grpc-smoke.local|identity.local" docs/deployment/envoy-gateway-local.md`

Expected:
- install command present
- wait command present
- apply commands present
- port-forward instructions present
- smoke-test grpcurl command present
- identity route note present

### Task 3: Final Verification Of The Envoy Deployment Deliverables

**Files:**
- Verify: `deployment/envoy-gateway/**`
- Verify: `docs/deployment/envoy-gateway-local.md`

- [ ] **Step 1: Verify file layout**

Run: `rg --files deployment/envoy-gateway docs/deployment | sort`

Expected:
- all six deployment YAML files exist
- `docs/deployment/envoy-gateway-local.md` exists

- [ ] **Step 2: Verify local route details and backend names are consistent**

Run: `rg -n "relay-system|relay-gateway|identity.local|grpc-smoke.local|grpc-smoke|ghcr.io/projectcontour/yages:v0.1.0|50051|9000" deployment/envoy-gateway docs/deployment/envoy-gateway-local.md`

Expected:
- namespace, Gateway, route hostnames, backend Service names, and ports line up across all manifests and docs

- [ ] **Step 3: Summarize what is usable now vs later**

The final implementation summary must state clearly:
- usable now: Helm install, Gateway API resources, smoke-test backend, port-forward, grpcurl verification
- later: real identity backend deployment plus final auth/rate-limit policy
