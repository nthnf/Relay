# Global Delivery Roadmap

## Phase 1: Platform Foundations
- Stand up a local kind cluster with namespaces, ingress pathing, and baseline developer workflow.
- Add local dependencies: Envoy Gateway, RabbitMQ, Redis for approved edge rate limiting plus approved `realtime` presence state, and per-service Postgres instances.
- Stand up Envoy Gateway as the backend ingress for the cluster and define baseline routing from externally reachable backend routes to internal gRPC services.
- Bootstrap per-service Postgres provisioning and migration conventions.
- Define and document the shared poll-based outbox worker pattern, including the standard `outbox_event` table contract and sidecar deployment shape.
- Deliver minimum delivery operations for RabbitMQ and outbox processing: bounded retry behavior, dead-letter routing, and baseline observability for publisher/consumer failures.

## Phase 2: Identity And Access
- Deliver `identity` service contracts for user identity, credential flows, token issuance, and service-to-service auth expectations.
- Define which backend gRPC surfaces are externally reachable by SvelteKit and which remain internal-only.
- Define Envoy Gateway access-token validation policy for protected routes and keep auth-entry routes explicitly public where required.
- Define `identity` refresh-token rotation semantics so refresh returns a new access token and a new refresh token while revoking the old refresh token.
- Preserve service-owned authorization and domain invariants inside backend services instead of shifting them into ingress policy.
- Define user account persistence in the identity-owned Postgres database.
- Document synchronous gRPC contracts needed for authenticated internal requests and other immediate authoritative decisions.
- Apply the Phase 1 retry, dead-letter, and observability conventions to identity-owned asynchronous flows before depending on them elsewhere.

## Phase 3: Friendship And Workspace Membership
- Deliver service contracts for friendship lifecycle and workspace membership lifecycle.
- Provision dedicated Postgres databases for each participating service with isolated schema ownership.
- Define RabbitMQ events and consumer responsibilities for membership-related eventual consistency.
- Document authorization dependencies between identity state, friendship state, and workspace membership checks.

## Phase 4: Chat Write Path And Realtime Delivery
- Deliver the `chat` write-path contract for message create flows and durable persistence.
- Define the `realtime` service interface for low-latency delivery and presence-related fanout.
- Document the allowed synchronous edge: `chat` may synchronously notify `realtime` only for low-latency message-create fanout, but `chat` remains the durable source of truth.
- Deliver durable propagation through RabbitMQ using the service-local standard table `outbox_event` and sidecar workers as the backup and recovery path.
- Specify idempotent consumer behavior for chat-related downstream updates.

## Phase 5: Bootstrap Query Projections
- Deliver the `bootstrap` service contract as the canonical UI-facing aggregated read service.
- Build projections in dependency order: identity/user basics first, then friendship and membership state, then chat conversation/message summaries needed by the UI.
- Define RabbitMQ consumer inputs and projection refresh behavior for `bootstrap`.
- Document how `bootstrap` serves projection-backed reads instead of runtime cross-service aggregation or ad hoc fanout reads for the same aggregate use cases.

## Phase 6: Email And Operational Hardening
- Deliver email workflow contracts for verification, recovery, and operational messaging.
- Extend the earlier retry, dead-letter, and observability baseline with stronger health checks and operator-facing diagnostics.
- Define rate-limiting policy backed by Redis where justified at the edge or identity boundaries, while preserving the approved `realtime` presence usage.
- Document backup, restore, and local disaster-recovery expectations for per-service Postgres and RabbitMQ development environments.
