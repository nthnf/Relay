# Friendship User Stories

## Send a friend request to another user

As an authenticated user, I can send a friend request to another existing user when we are not already friends, there is no active block in either direction, and there is no existing pending request for the pair, so the system records one durable pending relationship request without creating orphaned relationship rows.

## Accept or reject a pending incoming request

As a user with an incoming pending request, I can accept it to create a mutual friendship or reject it to close the request, and the service publishes the corresponding durable event for downstream consumers such as `bootstrap`.

## Remove an existing friend relationship

As an authenticated user, I can remove an accepted friend so the system deletes the symmetric friendship edges and downstream accepted-friend projections can converge away from that pair.

## Block another user and prevent normal friend interactions

As an authenticated user, I can block another existing user so friend requests cannot be created or accepted for that pair while the block exists, and any existing friendship or pending requests between us are cleared by the write owner with durable request-closure visibility.

## Unblock without restoring previous state automatically

As an authenticated user, I can unblock another user later, but the system does not silently restore deleted friendship edges or prior requests; a new request flow must start if we want to reconnect.
