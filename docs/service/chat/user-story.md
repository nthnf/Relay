# Chat User Stories

## Open the app and load my inbox

As a signed-in user, I open one websocket to `realtime` for live delivery, then fetch history, channel lists, and DM state from `chat`, `workspace`, or `bootstrap` so durable data stays on source-of-truth services.

## Send a message to a workspace channel

As an active workspace member, I can send a message to a channel so chat durably stores the message as the source of truth, assigns conversation-scoped ordering, and then attempts low-latency realtime fanout without making delivery a prerequisite for write success.

## Create a workspace channel and initialize its chat conversation

As an active workspace member, when I create a new channel the application creates the workspace channel first and then immediately calls chat once to create its durable `conversation_id`, so later message send and history flows can use that stable chat-owned conversation address.

## Start or reopen a direct message with another user

As an authenticated user, I can press a dedicated DM-create button for another user so chat creates a stable 1:1 `conversation_id` once and persists messages for that conversation without depending on workspace channel state.

## Edit or delete my own message

As author of my own message, I can edit or delete it so chat preserves durable current state, records soft-delete and latest-edit metadata, and lets downstream consumers converge from chat-owned events.
