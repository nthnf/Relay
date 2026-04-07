## Identity Documentation Roadmap

1. Define `user_account`, `user_profile`, `user_credential_password`, `user_session`, and `email_verification_token` with clear UUID keys, strict `email_normalized` uniqueness, account-status enforcement, expiry, and relation rules.
2. Define hot-path authentication, email-verification redemption, profile-update, and profile lookup RPCs used by external application servers through Envoy Gateway and approved internal callers.
3. Define durable registration, profile-change, email-verification, and session-revocation events published through the identity outbox.
4. Document the local `outbox_event` integration, disable-account session-revocation effects, and expected downstream consumers such as `bootstrap`.
