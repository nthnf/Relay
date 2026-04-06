## Implementation Order

1. Define home, accepted-friend, workspace, channel, and unread projection tables with contractual sort keys.
2. Define bootstrap read routes and denormalized response shapes, including `200` versus `404` behavior for lagging projections.
3. Map upstream create and update events to projection updates, including mutable display-field refresh events.
4. Document consistency tradeoffs, local projection-based unread computation, replay expectations, and the projection rebuild workflow.

## Delivery Notes

- Keep bootstrap read-only for cross-domain aggregates.
- Prefer projection reads over ad hoc runtime fanout for aggregates owned by bootstrap.
- Keep contracts aligned with gateway-facing UI payload needs and eventual consistency.
