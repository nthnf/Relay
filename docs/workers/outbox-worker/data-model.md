## Canonical `outbox_event` Contract

All publishing services use the shared table name `outbox_event` with the same core column semantics so the sidecar worker behaves consistently across service boundaries. Services still own their local database and event payload contracts.

## Minimum Shared Constraints And Index Guidance

- `event_id` must be unique within a service's `outbox_event` table.
- Services should enforce a unique constraint or unique index on `event_id`.
- Services should add an index that supports polling eligible rows by `status` plus `available_at`.
- Services should add an index that supports lease recovery queries over `status` plus `claimed_at`.
- If services partition or otherwise optimize the table, they must preserve these lookup paths and the shared column semantics.

| Column             | Type          | Notes                                                                                     |
| ------------------ | ------------- | ----------------------------------------------------------------------------------------- |
| `event_id`         | `uuid`        | Stable unique event identifier. Also used by consumers for deduplication.                 |
| `aggregate_type`   | `text`        | Publisher-defined aggregate kind, such as `message` or `membership`.                      |
| `aggregate_id`     | `uuid`        | Aggregate instance identifier associated with the event.                                  |
| `event_type`       | `text`        | Versioned integration event type name owned by the publishing service.                    |
| `payload`          | `jsonb`       | Event body published to RabbitMQ. Must be self-contained enough for downstream consumers. |
| `status`           | `text`        | Lifecycle state: `pending`, `claimed`, `published`, or `failed`.                          |
| `publish_attempts` | `integer`     | Count of publish attempts started by the worker.                                          |
| `occurred_at`      | `timestamptz` | Domain event time from the source transaction.                                            |
| `available_at`     | `timestamptz` | Earliest time the row is eligible for polling or retry.                                   |
| `claimed_by`       | `text`        | Worker identity that currently holds the claim lease. Null when unclaimed.                |
| `claimed_at`       | `timestamptz` | Time the current claim was taken.                                                         |
| `published_at`     | `timestamptz` | Time the worker recorded successful publish completion.                                   |
| `last_error`       | `text`        | Most recent publish failure summary for operators and retry logic.                        |
| `created_at`       | `timestamptz` | Row creation time in the local service database.                                          |

## Semantic Rules

- `event_id` must be generated once and never changed for the lifetime of a row. The same identifier must survive normal retries.
- `status` transitions follow this shared contract:
  - `pending` -> `claimed` when a worker acquires a lease.
  - `claimed` -> `published` when publish success is durably recorded.
  - `claimed` -> `pending` for retryable failures by clearing or overwriting the claim fields and advancing `available_at`.
  - `claimed` -> `failed` when retry policy is exhausted or the row requires operator attention.
  - `failed` is terminal for automatic retry and changes only through explicit operator action.
- `publish_attempts` increments on each attempt to publish, not only on terminal failures.
- `available_at` is the scheduling field for initial availability and delayed retry.
- `claimed_by` and `claimed_at` support bounded leasing so another worker instance can recover abandoned claims.
- A row in `claimed` state is eligible for re-claim when its lease timeout has expired according to worker configuration.
- Re-claim overwrites `claimed_by` and `claimed_at` with the identity and timestamp of the new worker.
- A worker may refresh its own lease by updating `claimed_at`; it must not refresh claims owned by another worker.

## Routing Contract

- `event_type` is the minimum logical routing signal that every service must provide.
- `headers` may include additional routing hints needed by shared worker configuration.
- Worker configuration defines the RabbitMQ exchange and publish strategy for the service, and must map row data consistently so all rows of the same documented event contract route the same way.
- Service docs must document any routing hints beyond `event_type`; they must not invent per-event routing rules that bypass the shared worker contract.

## Idempotency And Retry Safety

- Publisher idempotency is anchored on `event_id`; retries must reuse the same row and identifier.
- A worker may face partial failure after RabbitMQ accepted a publish but before `published_at` was recorded. Re-attempting that row must be considered safe and may produce duplicate delivery.
- Consumer-facing contracts must therefore treat `event_id` as the deduplication key.

## Replay And Recovery Contract

- Normal retry operates on the existing row and preserves the same `event_id`.
- Operator-initiated replay creates a new outbox row with a new `event_id`.
- Replay rows should include metadata in `headers` pointing to the original event, such as the original `event_id` and replay reason.
- Replay does not mutate the original published row; it creates a new publish attempt lineage for auditability and downstream deduplication clarity.

## Why Semantics Stay Consistent Across Services

- The worker is shared infrastructure guidance used by multiple services; stable column meaning prevents per-service worker forks.
- Consistent semantics keep operational tooling, replay procedures, and recovery behavior uniform across the platform.
- Shared semantics do not break ownership boundaries: each service still owns its database, payload schema, and event taxonomy.
