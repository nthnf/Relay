# Workspace User Stories

## Create a workspace and become its first member

As an authenticated user, I can create a workspace and immediately become its first active member so the system durably establishes the workspace container, my membership, and the initial downstream projection seed in one write-owner transaction.

## Invite another user to join the workspace

As a workspace member with invite permission, I can issue an invitation for another existing user so the system records a durable invitation with explicit expiry and later turns that invitation into membership only when the invited user accepts it.

## Browse channels that belong to a workspace

As an active workspace member, I can list the channels that belong to my workspace so the client can render the ordered sidebar from workspace-owned channel metadata without treating chat messages as channel ownership state.

## Remove a member and revoke their workspace access

As a workspace administrator, I can remove a member so the system durably revokes their membership and downstream consumers can remove workspace visibility and connected-session access.

## Leave a workspace when I am not the owner

As a non-owner workspace member, I can self-remove from a workspace so my membership is durably revoked without requiring an administrator action, while the workspace owner remains non-removable in v1 until ownership transfer exists.
