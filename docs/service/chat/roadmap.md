## Chat Documentation Roadmap

1. Define `direct_conversation`, `direct_conversation_member`, `chat_message`, `chat_message_edit`, and `chat_message_reaction` with clear ownership, target-kind invariants, target-scoped ordering, edit-history tracking, soft-delete behavior, and reaction uniqueness rules.
2. Define primary message write and history read RPCs for create, edit, delete, channel list, DM conversation lookup/create, DM history list, add-reaction, and remove-reaction flows.
3. Define the synchronous realtime fanout contract so post-commit low-latency delivery is explicit while durable write success remains chat-owned.
4. Define durable chat events that cover both workspace-channel and direct-message targets for projections, replay, and recovery through the transactional outbox and RabbitMQ publication path.
