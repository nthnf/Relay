# Microservice Contract Documentation Design

## Goal

Define a documentation system for this Discord-like microservices project so the repository can serve as the initial source of truth for service boundaries, Postgres data contracts, sync and async communication contracts, worker behavior, and implementation roadmaps.

## Scope

This design covers:

- global architecture documentation
- per-service contract documentation under `docs/service/<service>/`
- shared worker documentation under `docs/workers/`
- project guidance in `AGENTS.md`
- global and per-service roadmaps

This design does not add application code. It produces documentation contracts intended to guide implementation.

## Repository Documentation Layout

The documentation set should use the following structure.

```text
AGENTS.md
docs/
  architecture.md
  roadmap.md
  service/
    <service>/
      README.md
      data-model.md
      openapi.yml        # HTTP-facing services only
      grpc.md            # gRPC-facing services only
      events.md
      diagram.md
      user-story.md
      roadmap.md
  workers/
    outbox-worker/
      README.md
      data-model.md
      events.md
      diagram.md
      roadmap.md
  superpowers/
    specs/
      2026-04-06-microservice-contract-docs-design.md
```

## Global Documentation Responsibilities

### `AGENTS.md`

`AGENTS.md` should act as core memory for future agents working in this repository. It should document:

- this is a Rust-heavy microservices project for a Discord-like application
- the assistant should help and guide throughout the project lifetime
- the assistant should not write application code unless clearly requested
- the assistant may write tests, infrastructure YAML, and documentation
- the assistant should ask for clarification instead of assuming when uncertainty is material
- service ownership and documentation standards should be preserved

### `docs/architecture.md`

This file should capture project-wide architectural rules:

- hot path uses synchronous communication with gRPC
- cold path uses asynchronous communication via RabbitMQ
- cold path is eventually consistent
- Traefik and the gateway service form the edge path
- local orchestration uses Kubernetes with kind
- each service owns its own Postgres database
- Redis is reserved for rate limiting or cache cases with concrete justification
- outbox publishing uses a sidecar worker with a poll-based strategy
- cross-service joins must not rely on foreign keys across databases

### `docs/roadmap.md`

This file should define the platform delivery order and cross-service milestones. It should be concise, actionable, and implementation-oriented.

## Service Documentation Responsibilities

Each service under `docs/service/<service>/` should describe a proposed v1 contract. These are intended to be concrete enough to implement against, while still allowing unresolved details to be marked with `[NEEDS CLARIFICATION]`.

### Required files per service

#### `README.md`

The service overview should define:

- service purpose
- domain ownership
- explicit non-goals
- upstream and downstream dependencies
- owned storage
- synchronous and asynchronous interfaces
- relationship to the shared outbox worker contract

#### `data-model.md`

This file should define the service's canonical Postgres contract, including:

- tables and purpose
- columns with SQL type, nullability, default, and constraint notes
- primary keys and unique constraints
- important indexes
- in-service relations
- cross-service references represented as plain IDs rather than foreign keys
- enum-like value sets or typed fields where relevant

#### `openapi.yml`

Only HTTP-facing services should receive `openapi.yml`. This should document concrete external or edge-facing HTTP routes, request and response schemas, auth expectations, and common error responses.

#### `grpc.md`

Services that participate in synchronous internal communication should define their gRPC contract in markdown. This file should describe:

- exposed RPC methods
- request and response shapes
- ownership constraints
- latency-sensitive use cases
- when gRPC is used instead of event-driven propagation

#### `events.md`

This file should define:

- published events
- consumed events
- event trigger conditions
- payload shape summary
- idempotency expectations
- ordering assumptions if any
- whether the event is used for projection, workflow continuation, fanout, or notification

#### `diagram.md`

This file should contain Mermaid syntax that explains data communication, not just static topology. Preferred forms are `flowchart` and `sequenceDiagram`.

The diagrams should show flows such as:

- incoming request path
- internal gRPC calls
- local database writes
- outbox writes
- outbox polling and publish path
- RabbitMQ consumption
- projection updates
- realtime fanout where applicable

#### `user-story.md`

This file should list concrete user stories tied to that service's owned responsibilities. Each story should be short, implementation-oriented, and useful as a delivery checklist.

#### `roadmap.md`

This file should list concrete deliverables in the intended implementation order for that service. The roadmap should favor small, testable milestones over large abstract phases.

## Worker Documentation Strategy

The worker design should be centralized as a shared contract under `docs/workers/outbox-worker/` rather than repeating per-service worker docs.

### Shared outbox worker principles

- the outbox worker is a sidecar pattern reused by service deployments
- each service owns its own `outbox_event` table inside its own Postgres database
- the outbox worker reads from the local service database, claims publishable events, and publishes them to RabbitMQ
- polling behavior is configurable through environment variables
- retry behavior and failure visibility are part of the documented contract

### Standard outbox schema direction

The shared worker documentation should define a consistent semantic contract for `outbox_event`, including fields such as:

- event identity
- aggregate identity
- aggregate type
- event type
- payload
- headers or metadata
- occurred timestamp
- publish status
- publish attempts
- last error
- claim or lock fields if needed for worker coordination

Exact field names may be adjusted during service doc authoring, but the semantics must remain consistent across services.

### Idempotency strategy

The worker contract must explicitly document idempotency across both sides of delivery:

- publisher-side retries must be safe when the outbox worker republishes after partial failure
- consumer-side processing must tolerate duplicate event delivery
- event identity should support deduplication and replay-safe processing

## Proposed Service Boundaries

The repository currently contains these service directories:

- `bootstrap`
- `chat`
- `email`
- `friendship`
- `gateway`
- `identity`
- `realtime`
- `workspace`

The proposed v1 documentation should use the following ownership model.

### `gateway`

- public edge application behind Traefik
- handles edge-facing HTTP and WebSocket entrypoints
- enforces auth context and rate-limiting policy at the boundary
- forwards hot-path requests to internal services over gRPC
- should remain thin and should not own core domain business state

### `bootstrap`

- UI-facing query service for fast initial page loads and aggregated fetches
- serves minimal, fast read models such as friend list, workspace list, unread counts, summaries, and profile context
- hot-path read surface backed by eventually consistent projections
- should not become a write authority for core domain entities

### `identity`

- owns user accounts, credentials, profile basics, sessions, and authentication-oriented state

### `friendship`

- owns friend requests, accepted friendships, block relationships, and state transitions around interpersonal relationships

### `workspace`

- owns workspaces, memberships, roles, invitations, and channel or room metadata

### `chat`

- owns messages, message edits, deletions, reactions, and message history state
- remains the source of truth for message persistence

### `realtime`

- owns WebSocket connection handling and low-latency client fanout
- supports direct synchronous coordination with `chat` for latency-sensitive message delivery
- can rely on event-driven propagation for less latency-sensitive state such as presence
- should not become the source of truth for durable chat data

### `email`

- owns outbound email intent handling, provider integration, delivery attempts, and email-related notification state

## Communication Model

The documentation should consistently reflect the following system behavior.

### Hot path

- client requests enter through Traefik and gateway-facing routes
- synchronous internal communication uses gRPC
- latency-sensitive flows, especially chat delivery to realtime fanout, may use direct service-to-service sync communication

### Cold path

- durable cross-service propagation uses RabbitMQ
- services record publishable events into `outbox_event` within the same local transaction boundary as domain writes when appropriate
- the outbox worker publishes events asynchronously
- downstream services update projections or secondary workflows using eventual consistency

### Bootstrap behavior

`bootstrap` is a hot-path query service but its data freshness may depend on eventually consistent projections. This tradeoff should be made explicit in the service documentation and user stories.

## Roadmap Strategy

The documentation should include both a global roadmap and a service-specific roadmap.

### Global roadmap direction

Recommended milestone ordering:

1. platform foundations
2. identity and access
3. friendship and workspace membership
4. chat write path and realtime delivery
5. bootstrap query projections
6. email notifications and operational hardening

### Per-service roadmap direction

Each service roadmap should generally move through:

1. schema and ownership definition
2. primary write and read contracts
3. gRPC interfaces for hot-path flows
4. outbox and event propagation
5. projection or downstream consumption impact
6. resilience and operational hardening

The steps should be phrased as concrete deliverables rather than generic intentions.

## Documentation Writing Rules

The contract authoring pass should follow these rules:

- prefer explicit proposed v1 contracts over vague placeholders
- use `[NEEDS CLARIFICATION]` only where a decision is materially ambiguous
- avoid inventing cross-database relational constraints
- keep service ownership crisp and avoid overlapping write authority
- use diagrams to explain request and event flow, not just dependency maps
- ensure user stories are specific enough to drive implementation slices

## Expected Deliverables From The Authoring Pass

The next authoring pass should produce:

- `AGENTS.md`
- `docs/architecture.md`
- `docs/roadmap.md`
- service documentation for each discovered service
- shared outbox worker documentation under `docs/workers/outbox-worker/`

## Known Design Flex Points

The authoring pass may still need to mark details with `[NEEDS CLARIFICATION]` in the generated service docs if any of the following remain materially ambiguous while writing:

- exact HTTP route naming for edge-facing services
- exact gRPC request and response field names
- exact table column naming in borderline cases
- durable versus ephemeral storage splits inside `realtime`
- whether `email` publishes downstream events after delivery attempts

These are acceptable flex points as long as the docs remain concrete enough to guide implementation.
