# AGENTS.md

## Project Memory

- Rust-heavy, Discord-like microservices platform.
- This repository is documentation-first until implementation is explicitly requested.
- Do not write application code unless clearly requested.
- Documentation, tests, and infrastructure YAML are allowed.
- Help and guide the project throughout its lifetime; keep this file current as the shared memory anchor.

## Core Architecture Contracts

- Synchronous service-to-service request/response flows use gRPC when the caller needs an immediate authoritative answer.
- Only selected flows, especially Envoy-side access-JWT validation on protected routes and `chat` to `realtime` message-create fanout, are truly latency-sensitive hot path.
- Cold path uses RabbitMQ for durable asynchronous propagation with eventual consistency.
- Browser traffic reaches an external SvelteKit application; backend traffic enters the Kubernetes cluster through Envoy Gateway.
- There is no custom backend `gateway` service by default in this topology.
- Envoy Gateway validates short-lived access JWTs for protected externally reachable backend routes before forwarding requests.
- `identity` owns refresh token rotation and issues new access and refresh token pairs when a refresh succeeds.
- Local orchestration uses Kubernetes via kind.
- Each service owns its own Postgres database; do not share service databases.
- Redis is allowed only for rate limiting, narrowly justified caching, or the approved `realtime` presence use.
- Durable event publishing uses a poll-based sidecar worker reading the service-local standard table `outbox_event`.
- `bootstrap` is the canonical aggregate read service when a consumer needs a composed cross-service read model.
- `chat` remains the durable source of truth for message writes.
- Synchronous `chat` to `realtime` notification for message-create fanout is a low-latency optimization, not a prerequisite for durable message write success.
- RabbitMQ plus `outbox_event` is the durable backup and recovery path for downstream fanout and projection repair.

## Working Rules For Agents

- Preserve service ownership boundaries.
- Preserve shared documentation conventions across services and platform docs.
- Ask for clarification instead of assuming when uncertainty materially affects the contract.
- Prefer concise, concrete, implementation-oriented documentation.
- Avoid inventing unrelated systems or platform components.

## Guidance For Agents

- Act as a technical mentor as well as an implementer: explain why a recommendation fits this repository, not only what to type.
- Stay concise, concrete, and recommendation-driven. When tradeoffs matter, give realistic pros and cons, then recommend one option instead of stopping at a neutral summary.
- Warn clearly when a proposal conflicts with documented architecture or ownership boundaries, including gRPC for synchronous authoritative flows, RabbitMQ for durable asynchronous propagation, service-local Postgres ownership, Redis restrictions, `bootstrap` as the canonical aggregate read service, and `chat` as the durable source of truth for messages.
- For libraries, frameworks, SDKs, APIs, CLI tools, and cloud services, use `find-docs` / Context7 first and prefer authoritative documentation over memory for version-sensitive guidance.
- If authoritative documentation is unavailable or insufficient, use web search as a fallback and say that you are doing so.
- Do not dump snippets without explanation; explain why the approach fits this project or why it conflicts with the documented contracts above.
