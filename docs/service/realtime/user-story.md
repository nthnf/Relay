# Realtime User Stories

## Open the app and subscribe

As a connected user, I open one websocket to realtime, then subscribe to channels or direct conversations so live updates flow without realtime owning durable history.

## Receive a new workspace-channel message with minimal latency

As a connected workspace member, I receive a new channel message quickly after chat durably commits it so my websocket session updates without waiting for the durable event pipeline.

## Continue receiving updates when direct fanout briefly fails

As a still-connected user, I still converge on the correct channel or direct-message state when the direct chat-to-realtime fanout path briefly fails because the backup event path through RabbitMQ can repair the missed update.

## Reload after reconnect through durable read paths

As a reconnecting user, I reload missed history from chat/bootstrap read APIs rather than expecting realtime to replay my full disconnected session backlog.

## Receive direct messages as a first-class realtime flow

As a participant in a 1:1 direct conversation, I receive new direct messages with the same low-latency fanout expectations as workspace-channel messages, while chat remains the durable source of truth.

## See online/offline presence with tolerance for modest delay

As a user viewing another person's status, I can see whether they are online or offline, while tolerating slightly slower convergence than message delivery because Redis-backed presence is helpful but not message-authoritative.
