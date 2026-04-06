## Purpose

The outbox worker is a reusable sidecar that polls a service-local `outbox_event` table and publishes eligible events to RabbitMQ. It provides the durable cold-path bridge between a service's committed Postgres writes and downstream eventual-consistency consumers.

## Deployment Model

- The worker is deployed as a sidecar pattern for each publishing service workload.
- Multiple worker instances against the same service-local `outbox_event` table are supported when that service workload is scaled horizontally.
- Each worker instance reads only that service's local Postgres database and coordinates through row claiming plus lease expiry.
- The worker publishes to RabbitMQ; it does not call downstream services directly.
- The sidecar deployment shape should be exercised in local kind and preserved in later environments.

## Responsibilities

- Poll the local `outbox_event` table for publishable rows.
- Claim rows safely for processing and recover expired claims from other worker instances.
- Publish claimed events to RabbitMQ.
- Record publish outcomes and retry metadata back into `outbox_event`.
- Support replay and recovery workflows without violating service ownership boundaries.

## Non-Goals

- Defining domain-specific event payloads for every service.
- Acting as a general job runner or workflow engine.
- Reading or mutating another service's database.
- Providing exactly-once delivery across RabbitMQ and consumers.

## Configuration

The worker is configured through environment variables. At minimum, configuration must cover:

- polling cadence
- batch size
- claim timeout or lease duration
- optional claim refresh interval when long-running publish batches require lease extension
- retry backoff behavior
- RabbitMQ connection and exchange/routing configuration
- worker identity used in `claimed_by`

## Failure Handling

- Rows remain durable in `outbox_event` until successfully published and recorded.
- Publish attempts must be retry-safe after partial failures, including cases where RabbitMQ accepted a message but the worker failed before marking the row published.
- Retryable failures return rows to `pending` with updated `publish_attempts`, `last_error`, and `available_at`.
- `failed` is reserved for operator attention or terminal handling after retry policy exhaustion; those rows are not retried automatically until an operator requeues or replays them.
- Expired `claimed` rows become eligible for re-claim by another worker instance.
- Duplicate delivery is an expected platform behavior; consumers must tolerate replay and duplicate publish outcomes.

## Relationship To Service Docs

- This document defines the shared worker contract and common operating model.
- Service docs remain responsible for domain event types, payload schemas, ordering expectations, and integration points where transactional writes also insert `outbox_event` rows.
- Shared routing responsibility is split intentionally: service-owned rows provide `event_type` and any routing hints in `headers`, while worker configuration defines the RabbitMQ exchange and publish strategy used to emit them.
- Service-specific docs may tighten guarantees, but they must not weaken the shared `outbox_event` semantics defined here.
