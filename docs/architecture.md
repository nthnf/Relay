# Shared Platform Architecture Contract

## System Shape
- System style: Rust-heavy microservices platform modeled after a Discord-like product.
- Browser traffic reaches an external SvelteKit application rather than entering the cluster directly.
- SvelteKit may call approved backend gRPC services from its server runtime through Envoy Gateway when it needs an immediate authoritative backend answer.
- Backend traffic enters the Kubernetes cluster through Envoy Gateway, which is the ingress and policy boundary for exposed backend routes.
- Synchronous internal calls use gRPC when the caller needs an immediate authoritative answer; only a subset are truly latency-sensitive.
- Durable cross-service propagation uses RabbitMQ on the cold path with eventual consistency.
- `bootstrap` is the canonical UI-facing aggregated read service for cross-domain, projection-backed queries.
- Runtime cross-service fanout reads are not the default pattern for aggregates owned by `bootstrap`.

## Edge Path
- Browser clients talk to the external SvelteKit application, not directly to backend services.
- Envoy Gateway is the backend ingress and north-south policy boundary for traffic that is allowed into the Kubernetes cluster.
- Envoy Gateway owns ingress concerns such as route exposure, ingress authentication policy, rate limiting, and traffic policy enforcement.
- Exposed backend routes may forward to internal gRPC services, but service-owned authorization and domain invariants still remain inside the destination services.
- There is no custom backend `gateway` service by default in this topology.

## Hot Path
- Use synchronous gRPC when the caller cannot wait for asynchronous convergence and needs an immediate authoritative answer.
- Not every gRPC call has the same latency target. `identity`, `friendship`, and `workspace` are primarily synchronous command and authorization services; they stay synchronous for correctness, permission checks, and immediate user feedback even when they do not require chat-like fanout latency.
- SvelteKit may issue synchronous server-side gRPC calls through Envoy Gateway to approved backend services when the application cannot wait for eventual consistency.
- Envoy Gateway may expose selected backend gRPC surfaces outward to approved external callers as the north-south ingress boundary, but exposure policy is distinct from the authorization decisions enforced by backend services themselves.
- Services may use direct gRPC when immediate authorization, correctness, or low latency is required and ownership boundaries remain clear.
- `chat` may synchronously notify `realtime` only for low-latency message-create fanout.
- `chat` remains the durable source of truth for message persistence and write acceptance.
- A synchronous notify failure from `chat` to `realtime` must not by itself make a durable message-create write fail once the `chat` write commits.
- The RabbitMQ path backed by the service-local standard table `outbox_event` is the durable backup and recovery path for fanout catch-up.
- Hot-path calls must not depend on cross-service database access.

## Cold Path
- Use RabbitMQ for durable, asynchronous cross-service propagation.
- Durable cross-service propagation uses RabbitMQ via the service-local standard table `outbox_event`.
- Consumers update their own state and projections independently.
- Cold-path handlers must be idempotent because delivery and replay are expected platform behaviors.
- Presence can rely more heavily on event-driven propagation than chat fanout, but both remain recoverable through the cold path.

## Data Ownership
- Each service owns its own Postgres database.
- Cross-service reads must happen through gRPC APIs or projection materialization, not shared tables.
- Schema changes must preserve service ownership boundaries and documented contracts.
- Per-service Postgres bootstrapping is required from the start so ownership remains explicit in local and later environments.

## Shared Outbox Pattern
- The shared standard outbox table name is `outbox_event`.
- Every service that publishes integration events persists them in its local `outbox_event` table within the same database transaction as the source write.
- A poll-based sidecar worker reads unpublished rows, publishes to RabbitMQ, and marks rows as dispatched.
- The sidecar pattern is shared infrastructure guidance, but each service still owns its table schema and event payload contract.
- Ordering guarantees are per publisher and should be documented per aggregate or stream when needed.

## Redis Usage Policy
- Redis is not a general shared state store.
- Approved defaults: rate limiting, narrowly justified caching, and the approved `realtime` online/offline presence store.
- Any cache use must define owner, keys, TTLs, invalidation behavior, and fallback behavior in docs before adoption.

## Local Kubernetes Topology
- Local orchestration uses kind.
- kind runs the backend ingress and service topology needed for end-to-end backend development.
- Default local dependencies include Envoy Gateway, RabbitMQ, per-service Postgres instances, allowed Redis usage, core services, Kubernetes Services for internal service discovery and load balancing, and supporting workers.
- The SvelteKit application remains external to the cluster even when it participates in local end-to-end flows.
- Sidecar outbox workers run alongside publishing services in local Kubernetes so the delivery pattern is exercised early.

## Consistency Model
- Inside a service boundary, writes are strongly consistent within that service's Postgres transaction.
- Across service boundaries, the platform is eventually consistent by default.
- `bootstrap` owns the canonical UI-facing aggregate read path for projection-backed cross-domain queries.
- Services should not introduce parallel runtime fanout-read implementations for the same UI aggregate use cases owned by `bootstrap`.
- Use synchronous gRPC for immediate authoritative decisions or truly low-latency flows that cannot wait for asynchronous convergence.

## Documentation Conventions
- Treat repository docs as proposed v1 platform contracts until superseded.
- Prefer explicit contracts over aspirational prose.
- Mark material ambiguity as `[NEEDS CLARIFICATION]` rather than inventing detail.
- Keep shared docs aligned with service ownership boundaries, transport choices, and the outbox pattern.
