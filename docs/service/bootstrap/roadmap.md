## Implementation Order

1. Define home, accepted-friend, workspace, channel, and unread projection tables with contractual sort keys.
2. Define bootstrap read RPCs and denormalized response shapes, including success versus not-found behavior for lagging projections.
3. Map upstream create and update events to projection updates, including mutable display-field refresh events.
4. Document consistency tradeoffs, local projection-based unread computation, replay expectations, and the projection rebuild workflow.

## Delivery Notes

- Keep bootstrap read-only for cross-domain aggregates.
- Prefer projection reads over ad hoc runtime fanout for aggregates owned by bootstrap.
- Keep contracts aligned with external SvelteKit UI payload needs, Envoy-routed exposure boundaries, and eventual consistency.
