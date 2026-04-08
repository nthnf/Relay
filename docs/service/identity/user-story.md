# Identity User Stories

## Register with a unique email and password

As a new user, I can register with an email, password, username, and display name so long as the normalized email is not already owned by another account, and the system hashes my password server-side, creates my active identity record, and starts the initial email verification flow in one durable write path.

## Verify my email with a single-use token

As a newly registered user, I can redeem the email verification token issued for my account so identity marks my email as verified exactly once and publishes a durable `UserEmailVerified` event for downstream consumers.

## Refresh expired access tokens by rotating the refresh token

As an authenticated user, I use short-lived access JWTs that Envoy Gateway validates on protected routes, and when the access token expires the external application server can call identity with my refresh token to receive a new access token and a new refresh token while the old refresh token is revoked.

## Keep client-bound refresh sessions tied to the issuing client instance

As a client using a refresh session that was issued with `client_instance_id`, I must present the same `client_instance_id` during refresh or the refresh attempt is treated as invalid.

## Update profile basics through the identity owner

As an authenticated user, I can update my display name and avatar URL through identity's bounded profile write contract so downstream projections receive a durable `UserProfileUpdated` event.

## Other services can resolve stable profile basics

As an internal service owner, I can resolve `user_id`, `username`, `display_name`, `avatar_url`, and email verification status through identity's stable contract, either via identity events for projections or bounded lookup RPCs when a direct owner read is justified.
