# Chat User Stories

## Send a message to a workspace channel

As an active workspace member, I can send a message to a channel so chat durably stores the message as the source of truth, assigns channel-scoped ordering, and then attempts low-latency realtime fanout without making delivery a prerequisite for write success.

## Start or reopen a direct message with another user

As an authenticated user, I can open a direct message with another user so chat returns a stable 1:1 `direct_conversation_id` and persists messages for that conversation without depending on workspace channel state.

## Edit or delete my own message

As the author of my own message, I can edit or delete it so chat preserves durable current state, records edit history or soft-delete metadata, and lets downstream consumers converge from chat-owned events.

## React to a message and see the reaction state update

As a channel participant, I can add or remove a reaction on a message so chat durably tracks unique per-user reactions and connected clients can observe the updated reaction state through low-latency fanout or durable catch-up.
