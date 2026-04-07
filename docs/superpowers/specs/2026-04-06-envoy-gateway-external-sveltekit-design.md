# Envoy Gateway With External SvelteKit Design

## Goal

Revise the platform edge architecture to match a new deployment model where the user-facing application is an external SvelteKit deployment and the Kubernetes cluster is backend-only. The design removes the custom backend `gateway` service as the protocol-translation layer, replaces Traefik with Envoy Gateway as the backend ingress and policy layer, and preserves the existing service ownership and async/sync backend contracts where they still apply.

## Scope

This design covers:

- replacing Traefik with Envoy Gateway as the backend ingress layer
- removing the custom backend `gateway` service from the north-south backend path
- treating SvelteKit as external to the cluster
- keeping backend services inside Kubernetes as gRPC and RabbitMQ-based microservices
- clarifying where ingress auth/policy ends and service-owned authorization begins

This design does not change core backend ownership rules such as service-local Postgres databases, `chat` as the durable message source of truth, or RabbitMQ as the cold-path propagation mechanism.

## New Topology

### External application layer

- SvelteKit is hosted outside the Kubernetes cluster, for example on Vercel.
- Browser clients talk to SvelteKit, not directly to backend services.
- SvelteKit acts as the user-facing application server and can issue gRPC calls from its server runtime.

### Backend cluster edge

- Envoy Gateway becomes the backend ingress and edge-policy layer.
- Envoy Gateway is responsible for backend ingress concerns such as:
  - public entrypoint into the backend cluster
  - reverse proxying
  - TLS termination
  - route matching
  - backend ingress rate limiting
  - authentication and external auth policy if adopted
- Envoy Gateway routes to Kubernetes Services for backend workloads.

### Backend services

- Backend services remain inside Kubernetes.
- Synchronous authoritative backend calls continue to use gRPC.
- Cold-path durable propagation continues to use RabbitMQ and `outbox_event`.
- Kubernetes Services continue to provide internal service discovery and in-cluster load balancing.

## Main Architectural Change

The current docs assume:

- Traefik as ingress
- a custom backend `gateway` service as the north-south application edge
- `gateway` handling protocol translation and auth-context attachment

The new design removes that assumption.

### New rule

- There is no custom backend `gateway` service by default.
- Envoy Gateway is the cluster ingress and policy boundary.
- SvelteKit is the external application edge and can communicate with backend services over gRPC from its server runtime.

## Why The Custom Gateway Is No Longer Needed

The previous custom `gateway` service was justified primarily as:

- protocol translation
- thin application edge routing
- edge-facing auth handling

With the new deployment model:

- SvelteKit already handles the user-facing application layer
- SvelteKit can speak gRPC from the server runtime
- Envoy Gateway can provide backend ingress, routing, and policy enforcement

That means a separate backend application gateway is no longer justified unless it owns real application behavior beyond transport translation.

## Recommended Edge Responsibility Split

### SvelteKit

- browser-facing web application
- external application edge
- server-side gRPC client to backend services
- user-facing orchestration and page/API composition

### Envoy Gateway

- backend ingress
- reverse proxy and route control
- TLS termination
- backend ingress rate limiting
- auth or ext-auth policy where appropriate
- routing to Kubernetes Services

### Backend services

- authoritative service-owned business logic
- service-owned authorization inside each service boundary
- gRPC request/response between backend services where immediate answers are required
- RabbitMQ for cold-path propagation

## Authorization Boundary

The new design should keep a strict distinction between:

### Ingress authentication and coarse policy

Envoy Gateway may enforce:

- token presence/shape checks
- short-lived access-token validation or ext-auth policies
- coarse ingress access policies
- backend ingress rate limits

### Service-owned authorization

Backend services must still enforce:

- workspace membership checks
- friendship block rules
- identity session semantics if they remain identity-owned
- chat/write ownership rules
- any domain-specific authorization invariants

Ingress auth must not replace service-owned business authorization.

## Exposure Model

Not every backend service should automatically become public just because SvelteKit is external.

The design should recommend:

- expose only the subset of backend gRPC services or routes that SvelteKit actually needs
- keep internal-only service APIs private to the cluster where possible
- document public backend ingress exposure explicitly rather than implying full external reachability for all services

## Kubernetes Role In The New Model

Kubernetes continues to provide:

- Deployments for backend workloads
- Services for stable DNS and internal load balancing
- Pod scaling and service discovery

Envoy Gateway continues to add what Kubernetes alone does not provide as a complete edge solution:

- ingress-controller behavior
- route and policy implementation
- public reverse proxy behavior
- edge middleware and auth integration

## What Stays The Same

The following architecture contracts remain valid:

- service-local Postgres ownership
- gRPC for synchronous authoritative backend flows
- RabbitMQ for durable asynchronous propagation
- `bootstrap` as the canonical aggregate read service if retained
- `chat` as the durable source of truth for messages
- `chat` to `realtime` message-create fanout as the selected truly latency-sensitive backend path

## Documentation Changes Required

The follow-up implementation should update docs to:

- replace Traefik with Envoy Gateway in shared architecture docs
- remove the custom backend `gateway` service as the north-south entrypoint from platform docs
- remove `docs/service/gateway/` or replace it only if a new backend service with real ownership still exists
- update diagrams that currently show `Traefik -> gateway -> service`
- update roadmap and service docs that assume the custom gateway remains in the path

## Success Criteria

The update is successful if the documentation clearly communicates that:

- SvelteKit is external to the cluster
- Envoy Gateway is the backend ingress and policy layer
- the custom backend `gateway` service is no longer required by default
- backend services still own their own business logic and authorization
- Kubernetes Services still provide internal service discovery and load balancing

## Risks And Tradeoffs

### Risk: Too many backend services become public by accident

Mitigation:

- document explicit public ingress exposure instead of assuming all services are externally reachable

### Risk: Ingress auth gets mistaken for domain authorization

Mitigation:

- explicitly state that service-owned authorization remains inside each service boundary

### Risk: SvelteKit becomes tightly coupled to too many backend services

Mitigation:

- expose only the gRPC services that SvelteKit actually needs
- preserve `bootstrap` or other bounded read surfaces where useful

## Implementation Note

The next implementation step should be a documentation update, not application code, unless explicitly requested.
