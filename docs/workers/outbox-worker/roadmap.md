## Implementation Order

1. Standardize `outbox_event` schema semantics across services.
2. Define worker polling, claiming, and retry environment variables.
3. Implement safe publish retry behavior, lease recovery, and duplicate tolerance rules.
4. Document service integration points for writing outbox rows and routing hints.
5. Document replay and recovery procedures, including requeue versus replay rules.

## Delivery Notes

- Complete the shared contract first so service-specific docs can inherit stable semantics.
- Keep the worker reusable and sidecar-scoped; do not expand it into unrelated background processing infrastructure.
- Validate every service integration against service-local Postgres ownership and RabbitMQ-based cold-path propagation.
