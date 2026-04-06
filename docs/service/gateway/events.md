## Published Events

- None required for v1.

Gateway should not become a domain event owner. Normal auth, chat, membership, and other workflow events belong to the service that owns the underlying state.

## Consumed Events

- None required for v1 request handling.

Gateway resolves hot-path decisions synchronously through internal service boundaries rather than building its own asynchronous domain state.

## Notes

- Realtime ticket issuance is synchronous and request-scoped; the v1 signed token contract does not require RabbitMQ publication.
- The v1 ticket is minted by `gateway`, targeted to audience `realtime`, expires 30 seconds after issuance, includes `jti` and `client_instance_id`, and is reusable until expiry because there is no persisted redemption store.
- Clients present the ticket during websocket connect as the `ticket` query parameter on the gateway websocket entrypoint.
- `POST /v1/auth/register` is the only v1 route with gateway-managed idempotency via the `Idempotency-Key` header and `gateway_idempotency_key` tracking.
- Gateway must normalize public errors to stable codes: `INVALID_REQUEST`, `UNAUTHENTICATED`, `CONFLICT`, `INVALID_IDEMPOTENCY_KEY`, `RATE_LIMITED`, `UPSTREAM_UNAVAILABLE`, and `INTERNAL_ERROR`.
- If later operational events are added for edge telemetry or security workflows, they must stay infrastructure-scoped and must not make gateway the owner of domain projections.
