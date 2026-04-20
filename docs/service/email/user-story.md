# Email User Stories

## Receive a verification email after registration

As a newly registered user, I receive a verification email after identity durably accepts my registration so the platform can guide me into the email-verification flow without making registration success depend on synchronous provider delivery.

## Inspect delivery attempts and failure reasons

As an operator, I can inspect outbound email records and their delivery attempts so I can see whether a verification email was submitted, retried, or failed and understand the latest provider-facing failure reason without depending on a published delivery-result event.
