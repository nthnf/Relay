## Publisher Guarantees

- The publisher writes the domain change and the corresponding `outbox_event` row in the same local Postgres transaction.
- The worker only reads the publishing service's local `outbox_event` table and publishes eligible rows to RabbitMQ.
- Multiple worker instances may publish from the same service-local table when the service deployment is horizontally scaled; row claims and lease expiry coordinate concurrency.
- Publisher retries must be safe after partial publish failures, including cases where the message may already exist in RabbitMQ.
- Polling cadence, batch size, and retry backoff are env-configurable.
- RabbitMQ routing uses a shared split of responsibility: worker configuration defines the exchange and publish strategy, while row data supplies `event_type` and any documented routing hints from `headers`.
- For every published event, the worker must surface `event_id` in the broker delivery envelope or headers so downstream consumers can deduplicate without reinterpreting payload-specific fields.

## Consumer Expectations

- Duplicate RabbitMQ delivery is possible.
- Consumers must deduplicate by `event_id`.
- Consumers must be idempotent for normal delivery, replay, and recovery scenarios.
- Consumers own their own state updates and projection repair logic.

## Idempotency Strategy

- `event_id` is the platform-wide deduplication key for a single integration event.
- Publishers preserve the same `event_id` across retries.
- The broker-delivered message for that event must carry the same `event_id` in transport metadata or headers on every retry.
- Operator replay creates a new outbox row with a new `event_id`, while preserving provenance to the original event in metadata.
- Consumers should persist processed `event_id` values or otherwise enforce equivalent idempotent handling within their own boundary.

## Retry Semantics

- Rows become eligible for retry based on `available_at`.
- Retry delay should be driven by env-configurable backoff settings.
- A retry may legitimately republish the same event if the earlier publish result was ambiguous.
- Retryable publish failures return the row to `pending`; `failed` is reserved for rows removed from automatic retry and awaiting operator action.
- Expired `claimed` rows become eligible for re-claim by another worker instance.
- Re-claim overwrites `claimed_by` and `claimed_at`; a worker may refresh only its own active claim lease.
- The shared contract prefers at-least-once delivery with duplicate tolerance over message loss.

## Failure Cases

- **RabbitMQ unavailable before accept**: leave or return the row to a retryable state and advance `available_at`.
- **RabbitMQ accepted publish but worker crashed before marking success**: the event may be published again on recovery; consumers must deduplicate by `event_id`.
- **Worker crashes while holding claims**: claim timeout or lease expiry should allow safe recovery by another worker instance, and expired `claimed` rows become re-claimable.
- **Poison event or repeated downstream rejection**: record `last_error`, preserve the row, and use documented replay/recovery procedures instead of silent loss.

## Replay And Recovery

- Normal retry is automatic worker behavior on the existing row and keeps the same `event_id`.
- Operator-initiated replay is a separate action that creates a new outbox row, a new `event_id`, and metadata linking the replay to the original event.
- Operators should prefer re-queueing an existing `failed` row when the original `event_id` should remain the deduplication key, and use replay when a new publish lineage is desired.
