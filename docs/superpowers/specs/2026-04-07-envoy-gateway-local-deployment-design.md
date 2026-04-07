# Envoy Gateway Local Deployment Design

## Goal

Create a usable local Envoy Gateway setup for this repository's backend-only Kubernetes cluster, with documentation that explains how to install it on `kind`, apply repo-managed Gateway API resources, port-forward it locally, and verify a working gRPC route before real backend services are fully deployed.

## Scope

This design covers:

- installing Envoy Gateway locally with Helm
- storing repo-managed Gateway API resources under `deployment/envoy-gateway/`
- exposing a backend gRPC route shape for the future `identity` service
- adding a temporary smoke-test gRPC backend so the routing path is testable immediately
- documenting local setup and usage under `docs/deployment/`

This design does not finalize the production auth model or production TLS posture.

## Context

- The approved platform architecture is now:
  - browser -> external SvelteKit
  - SvelteKit server runtime -> Envoy Gateway -> approved backend gRPC surfaces
  - no custom backend `gateway` service by default
- The Kubernetes cluster is backend-only.
- `kind` is the local orchestration choice.
- The user wants Envoy Gateway configuration that is usable locally, not just conceptual.
- The user selected:
  - Helm install + repo YAML
  - `kubectl port-forward` for local access

## Recommended Approach

Use a split model:

1. install Envoy Gateway with Helm into its own control namespace
2. commit the project-specific Gateway API resources into this repo
3. include one temporary smoke-test gRPC backend so the ingress path can be verified immediately
4. include a project-shaped `GRPCRoute` for `identity` so the repo already reflects the intended backend exposure model

This gives a usable local ingress path now without blocking on a real deployable `identity` container image.

## File Layout

### Deployment resources

Create:

- `deployment/envoy-gateway/namespace.yaml`
- `deployment/envoy-gateway/gateway.yaml`
- `deployment/envoy-gateway/grpcroute-identity.yaml`
- `deployment/envoy-gateway/security-policy-placeholder.yaml`
- `deployment/envoy-gateway/backend-traffic-policy-placeholder.yaml`
- `deployment/envoy-gateway/smoke-test-grpc-backend.yaml`

### Deployment docs

Create:

- `docs/deployment/envoy-gateway-local.md`

## Resource Responsibilities

### `namespace.yaml`

Defines the backend application namespace used by the local platform manifests, for example `relay-system`.

### `gateway.yaml`

Defines the repo-owned `Gateway` resource that Envoy Gateway will reconcile.

Recommended local shape:

- one listener
- port `8080`
- protocol `HTTP`
- routes limited to the same namespace unless a broader route-sharing rule is needed later

The local design intentionally starts without TLS to keep the first setup usable with minimal friction.

### `grpcroute-identity.yaml`

Defines the intended project-facing gRPC route shape for the future `identity` backend Service.

The route should:

- attach to the repo-owned `Gateway`
- use a hostname such as `identity.local`
- route to the Kubernetes `Service` named `identity` on its gRPC port

This route may not be immediately testable until the real `identity` Deployment/Service exists, but it should still be present to codify the intended backend exposure model.

### `smoke-test-grpc-backend.yaml`

Defines a temporary gRPC backend Deployment and Service so the ingress path can be proven immediately.

The smoke-test backend should:

- live in the backend namespace
- expose a gRPC port
- be simple and disposable
- support verification with `grpcurl`

This resource exists to make the local Envoy setup usable now.

### `security-policy-placeholder.yaml`

Documents where ingress auth or ext-auth policy will attach later.

This file should be clearly marked as a placeholder rather than pretending the final auth model is already decided.

### `backend-traffic-policy-placeholder.yaml`

Documents where traffic policy such as timeouts, retries, or rate limiting will attach later.

This file should stay minimal and avoid inventing final production values.

### `docs/deployment/envoy-gateway-local.md`

Documents:

- prerequisites
- Helm install steps for Envoy Gateway
- applying repo manifests
- port-forwarding the Envoy service locally
- verifying the smoke-test route with `grpcurl`
- noting how the `identity` route becomes usable once the real backend Service exists

## Transport Design

For local development, use:

- `Gateway` listener protocol `HTTP`
- gRPC over cleartext HTTP/2 locally
- `kubectl port-forward` to reach Envoy Gateway from the workstation

This avoids introducing local certificate management before the routing model is proven.

TLS can be layered in later without changing the basic route structure.

## Verification Model

The local deployment is considered usable if a developer can:

1. install Envoy Gateway with Helm
2. apply the repo-managed namespace, Gateway, route, and smoke-test backend YAML
3. port-forward Envoy Gateway locally
4. send a gRPC request through Envoy Gateway to the smoke-test backend successfully

The `identity` route is considered structurally ready if it attaches correctly and points at the intended backend Service name and port, even if the real service image is not yet deployed.

## Non-Goals

- finalizing the production auth model
- finalizing production TLS and certificates
- exposing every backend service publicly
- replacing service-owned authorization with ingress-only policy

## Risks And Mitigation

### Risk: The setup is "usable" only in theory

Mitigation:

- include a real smoke-test gRPC backend so the path can be verified immediately

### Risk: Auth appears more complete than it is

Mitigation:

- keep policy manifests explicitly labeled as placeholders
- document clearly that service-owned authorization remains in backend services

### Risk: Backend exposure becomes too broad

Mitigation:

- include only the intended `identity` route pattern plus the smoke-test route
- document explicit allowlisting as the design default

## Success Criteria

The implementation is successful if:

- Envoy Gateway can be installed locally with Helm
- repo-managed Gateway API resources can be applied without guesswork
- a developer can verify an actual gRPC route locally via Envoy Gateway
- deployment docs under `docs/deployment/` are sufficient to reproduce the setup

## Implementation Note

The next step should implement deployment YAML and local deployment docs, not application code.
