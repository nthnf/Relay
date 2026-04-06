## Friendship Documentation Roadmap

1. Define `friend_request`, `friendship_edge`, and `user_block` with clear UUID references, directional-versus-symmetric semantics, and explicit blocking precedence.
2. Define request lifecycle and relationship-management RPCs for create, accept, reject, remove, block, unblock, and bounded owner reads.
3. Define durable relationship events needed by `bootstrap` and other consumers, especially accepted-friend creation and friendship removal signals.
4. Document blocking rules, target-user existence validation, duplicate-request handling, and the transactional outbox pattern used to publish relationship changes.
