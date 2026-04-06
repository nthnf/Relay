## Email Documentation Roadmap

1. Define `outbound_email` and `email_delivery_attempt` with clear idempotency, provider-traceability, retry-state, and operator-inspection rules.
2. Define self-contained consumed event triggers for verification-email and workspace-invitation flows driven by `UserRegistered` and `WorkspaceInvitationIssued`, including verification-link and invitation-link material needed at send time.
3. Define the provider abstraction boundary, provider handoff recording, duplicate-consume no-op behavior, and concise retry notes for retryable versus terminal submission failures.
4. Document the consume-only v1 boundary, the absence of durable delivery-result events, and the operational inspection flow through local email tables.
