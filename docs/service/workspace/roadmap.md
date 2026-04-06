## Workspace Documentation Roadmap

1. Define `workspace`, `workspace_member`, `workspace_role`, `workspace_member_role`, `workspace_invitation`, and `workspace_channel` with clear UUID ownership, membership uniqueness, invitation expiry, and channel-within-workspace uniqueness rules.
2. Define workspace and channel management RPCs for create, get, list, add-member, remove-member, invite, accept-invitation, and channel listing flows.
3. Define durable workspace invitation, membership, and channel events needed by `bootstrap`, `realtime`, and other downstream consumers.
4. Document permission-sensitive edge cases, including owner-versus-role authority, owner-removal restrictions, invitation-source membership attribution, create-time channel ordering rules, idempotency, invitation-expiry handling, and downstream projection update responsibilities driven by the transactional outbox pattern.
