# Identity User Stories

## Register with a unique email and password

As a new user, I can register with an email, password, username, and display name so long as the normalized email is not already owned by another account, and the system creates my active identity record, initial session, and initial email verification token in one durable write path.

## Verify my email with a single-use token

As a newly registered user, I can redeem the email verification token issued for my account so identity marks my email as verified exactly once and publishes a durable `UserEmailVerified` event for downstream consumers.

## Keep a valid session until it expires or is revoked

As an authenticated user, I keep using my session until its `expires_at` passes or identity revokes it, and the gateway can verify that session through identity without reading shared database state; if my account is disabled, identity invalidates my active sessions and future verification fails.

## Keep client-bound sessions tied to the issuing client instance

As a client using a session that was issued with `client_instance_id`, I must present the same `client_instance_id` during verification or the session is treated as invalid.

## Update profile basics through the identity owner

As an authenticated user, I can update my display name and avatar URL through identity's bounded profile write contract so downstream projections receive a durable `UserProfileUpdated` event.

## Other services can resolve stable profile basics

As an internal service owner, I can resolve `user_id`, `username`, `display_name`, `avatar_url`, and email verification status through identity's stable contract, either via identity events for projections or bounded lookup RPCs when a direct owner read is justified.
