# Gateway User Stories

## Authenticate Once And Reuse Edge-Issued Auth Context

As an authenticated client, I want to log in once and present a bearer token on later requests so that gateway can apply auth context consistently before forwarding hot-path calls to internal services.

For `POST /v1/auth/register`, I can also send `Idempotency-Key` so a safe retry replays the original response instead of creating ambiguous duplicate edge behavior.

## Request A Realtime Connection Ticket Before Opening A Websocket Session

As an authenticated client, I want to call `POST /v1/realtime/tickets` with a `client_instance_id` and receive a 30-second signed token containing my actor identity, session identity, `client_instance_id`, and `jti` so that I can open a browser-usable websocket connection by passing `?ticket=...` without gateway persisting ticket state in its database for v1.

## Receive Consistent HTTP Error Shapes When Upstream Services Reject Requests

As a client application, I want gateway to return a stable error envelope and normalized public codes such as `INVALID_REQUEST`, `UNAUTHENTICATED`, `RATE_LIMITED`, and `UPSTREAM_UNAVAILABLE` when an upstream service rejects my request so that I can implement predictable user-facing and retry behavior at the edge.
