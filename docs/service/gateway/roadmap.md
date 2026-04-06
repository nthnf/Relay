## Implementation Order

1. Define auth route contracts and the shared gateway error envelope.
2. Define the realtime ticket creation contract for the v1 signed token handshake, including claims, 30-second TTL, and websocket query-parameter presentation.
3. Add idempotency handling for `POST /v1/auth/register` using `Idempotency-Key` and `gateway_idempotency_key` replay semantics.
4. Document proxy boundaries, auth-context propagation, and upstream ownership.

## Delivery Notes

- Keep gateway thin and edge-scoped.
- Do not move domain persistence or async workflow ownership into gateway.
- Keep rate limiting in Redis and keep websocket ticket state out of required gateway Postgres persistence for v1.
