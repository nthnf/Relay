## Persistence Scope

Gateway keeps only edge-operational state and does not own domain records. User identity, credentials, membership, chat data, and realtime session state remain service-owned outside the gateway boundary.

## Postgres

Gateway Postgres is optional but approved for minimal operational tables where request safety requires durable coordination.

### `gateway_idempotency_key`

Use this table only for selected mutating public routes where clients may retry the same request and expect a stable response envelope.

V1 scope:

- Supported route: `POST /v1/auth/register`
- Request header: `Idempotency-Key`
- Registration is unauthenticated, so idempotency is keyed by the client-supplied `Idempotency-Key` at the gateway boundary with `actor_id = null`
- Replay behavior: same key + same request hash replays the original HTTP status and response body
- Conflict behavior: same key + different request hash returns `409 INVALID_IDEMPOTENCY_KEY`

| Column | Type | Notes |
| --- | --- | --- |
| `key` | `text` | Client-supplied idempotency key. |
| `actor_id` | `uuid null` | Authenticated actor bound to the key when the route is authenticated; `null` for unauthenticated registration in v1. |
| `request_hash` | `text` | Stable hash of method, route, and canonical request body. |
| `response_status` | `integer` | Final HTTP status returned for the accepted request. |
| `expires_at` | `timestamptz` | Time after which the key may be discarded. |
| `created_at` | `timestamptz` | Row creation time. |

### Semantic Rules

- For `POST /v1/auth/register`, uniqueness should be enforced on `key` alone because the route is unauthenticated and stores `actor_id = null`.
- If gateway later adds authenticated idempotent routes, uniqueness may be enforced on `key` within authenticated actor scope for those route contracts.
- `request_hash` must match on replay; a reused key with a different request shape must be rejected with `409 INVALID_IDEMPOTENCY_KEY`.
- Gateway stores only enough data to recognize a safe retry and replay the original response contract; domain write results still come from the upstream owner.
- `response_status` is the original HTTP status returned for the accepted request and is the minimum replay contract required by the table.
- Retention should stay short-lived and operational, not archival.

## Realtime Ticket State

For v1, websocket admission uses a short-lived signed token. Ticket state is not persisted in the gateway database as a required lookup table.

V1 signed token contract:

- Format: compact signed token minted by `gateway` and validated by `realtime`
- TTL: 30 seconds from issuance; implementations should not exceed 60 seconds without an explicit contract revision
- Audience: `realtime`
- Issuer: `gateway`
- Required claims:
  - `iss`: `gateway`
  - `aud`: `realtime`
  - `sub`: authenticated actor identifier
  - `session_id`: current auth session identifier
  - `client_instance_id`: client instance identifier from the ticket request
  - `jti`: unique ticket identifier for observability and future replay controls
  - `iat`: issued-at timestamp
  - `exp`: expiry timestamp
- Reuse model: reusable until `exp`; v1 does not enforce single-use because there is no persisted redemption store
- Presentation model: browser clients present the signed token during websocket connect as a query parameter on the gateway websocket entrypoint
- Binding rule: `client_instance_id` is mandatory for ticket creation and must match the instance opening the websocket session

## Redis

Redis is reserved for rate limiting and other explicitly justified ephemeral edge controls. It is not the source of truth for domain state and does not replace service-owned Postgres boundaries.
