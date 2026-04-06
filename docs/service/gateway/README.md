## Purpose

The gateway is the platform's thin public edge service. It terminates public HTTP and WebSocket entrypoints, applies identity-derived auth context, enforces edge-facing policy, and forwards hot-path requests to internal services over gRPC.

## Owned Responsibilities

- Terminate client-facing HTTP at the north-south edge after Traefik routing.
- Terminate the initial realtime connection handshake and mint short-lived signed tokens for v1 websocket admission.
- Validate bearer credentials with the identity boundary and attach resolved actor context to forwarded requests.
- Apply edge concerns such as request shaping, consistent error envelopes, idempotency handling for selected routes, and rate limiting.
- Route synchronous requests to the correct internal service without taking ownership of domain decisions.

## Non-Goals

- Owning user, workspace, friendship, chat, or other core domain records.
- Acting as a durable event publisher for domain workflows.
- Replacing service-owned authorization logic with gateway-only business rules.
- Serving as a general shared cache or session store.

## Dependencies

- **Traefik** for public ingress routing into `gateway`.
- **identity** service for registration, login, logout, token validation, and current-user lookup.
- **realtime** service for websocket session admission using the gateway-issued signed token contract.
- **Redis** for edge rate limiting only.

## Storage

- Minimal Postgres persistence is allowed only for edge-operational state such as idempotency records.
- Redis is allowed for rate limiting and narrowly justified ephemeral edge controls.
- Gateway does not own domain tables and does not persist v1 realtime ticket state as a required database lookup table.

## HTTP Surface

- `POST /v1/auth/register`
- `POST /v1/auth/login`
- `POST /v1/auth/logout`
- `GET /v1/me`
- `POST /v1/realtime/tickets`

All public HTTP contracts are documented in `openapi.yml`.

### V1 Idempotency

- `POST /v1/auth/register` is the only v1 route that accepts `Idempotency-Key`.
- Clients send the key in the `Idempotency-Key` HTTP header.
- Registration is unauthenticated, so v1 keys idempotency at the gateway boundary by the client-supplied `Idempotency-Key` itself with no authenticated actor binding.
- For this route, `actor_id = null` in `gateway_idempotency_key`.
- Gateway binds the key to the canonical request hash for the registration payload.
- A repeated request with the same key and the same request hash replays the original HTTP status and response body.
- A repeated request with the same key and a different request hash returns `409` with code `INVALID_IDEMPOTENCY_KEY`.

### V1 Error Normalization

Gateway returns a stable client-facing error envelope even when upstream services use different internal error shapes. The v1 public mapping is:

- `400 INVALID_REQUEST` for malformed JSON, schema validation failure, or unsupported request shape.
- `401 UNAUTHENTICATED` for missing, expired, or invalid bearer credentials.
- `409 CONFLICT` for domain conflicts exposed publicly, including duplicate registration conflicts.
- `409 INVALID_IDEMPOTENCY_KEY` for idempotency key reuse with a mismatched request hash.
- `429 RATE_LIMITED` for edge throttling.
- `503 UPSTREAM_UNAVAILABLE` when a required upstream service cannot be reached or times out at the gateway boundary.
- `500 INTERNAL_ERROR` for unexpected gateway failures.

### V1 Realtime Ticket Contract

- `POST /v1/realtime/tickets` returns a signed token, not a persisted lookup record.
- The ticket TTL is 30 seconds in v1.
- Required claims are `iss`, `aud`, `sub`, `session_id`, `client_instance_id`, `jti`, `iat`, and `exp`.
- `iss` identifies the gateway service; `aud` is `realtime`.
- The token is reusable until expiry in v1 because there is no redemption store.
- `client_instance_id` is required and the realtime handshake must reject use of the ticket with a different client instance.
- Browser clients present the ticket as a websocket query parameter during connect because arbitrary websocket headers are not broadly available in browsers.

## Asynchronous Interfaces

- None by default for v1 domain behavior.
- Gateway may consume infrastructure signals needed for safe edge operation in later phases, but it should stay thin and avoid async domain ownership.
