## Realtime Documentation Roadmap

1. Define websocket fanout responsibilities, ephemeral session/subscription routing state, authorization convergence, and the rule that realtime is a delivery service rather than the durable authority for messages or membership.
2. Define low-latency `PublishChannelMessage` and `PublishDirectMessage` gRPC contracts used by chat after durable write success, including `event_id` dedupe and target-scoped ordering rules.
3. Define backup event consumption behavior for durable chat and workspace events so active-session repair is explicit while reconnect catch-up stays on chat/bootstrap read paths.
4. Define Redis-backed online/offline presence handling, session tracking, and any remaining minimal durable Postgres recovery state.
