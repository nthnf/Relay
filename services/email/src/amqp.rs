use crate::entity::{email_delivery_attempt, outbound_email};
use crate::events::{EmailEvent, VerificationEmailRequested};
use crate::smtp::{SmtpClient, SmtpError};
use chrono::{DateTime, Utc};
use relay_amqp::{
    DeliveryContext, EventHandleError, EventHandleResult, RegisteredSubscriber,
    RegistersAmqpRoutes, route,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter,
    Set,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct Handler {
    db: DatabaseConnection,
    public_web_base_url: String,
    smtp_provider_name: String,
    smtp: SmtpClient,
}

pub use Handler as AmqpHandler;

impl Handler {
    pub fn new(
        db: DatabaseConnection,
        public_web_base_url: String,
        smtp_provider_name: String,
        smtp: SmtpClient,
    ) -> Self {
        Self {
            db,
            public_web_base_url,
            smtp_provider_name,
            smtp,
        }
    }

    pub(super) async fn handle_verification_email_requested(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: VerificationEmailRequested,
    ) -> EventHandleResult {
        let dedupe_key = format!(
            "verification_email:{}:{}",
            payload.verification_token_id, payload.reason
        );
        let source_event_id =
            self.source_event_id("VerificationEmailRequested", &payload, &delivery);
        let source_occurred_at = parse_timestamp(&payload.requested_at)
            .map_err(|e| EventHandleError::Permanent(format!("invalid requested_at: {e}")))?;
        let verification_url = format!(
            "{}/verify-email?token={}",
            self.public_web_base_url.trim_end_matches('/'),
            payload.verification_token
        );
        let subject = "Verify your Relay account".to_string();
        let body_text = format!(
            "Hi,\n\nVerify your email for Relay by visiting:\n{verification_url}\n\nThis link expires at {}.\n",
            payload.verification_token_expires_at
        );
        let body_html = Some(format!(
            "<p>Hi,</p><p>Verify your email for Relay by visiting <a href=\"{verification_url}\">this link</a>.</p><p>This link expires at {}.</p>",
            payload.verification_token_expires_at
        ));

        let outbound = self
            .insert_outbound_email(NewOutboundEmail {
                dedupe_key,
                email_kind: "registration_verification".to_string(),
                recipient_user_id: Some(parse_uuid(&payload.user_id, "user_id")?),
                recipient_email: payload.email.clone(),
                template_key: "verify-email-v1".to_string(),
                template_version: 1,
                subject,
                body_text,
                body_html,
                source_event_type: "VerificationEmailRequested".to_string(),
                source_event_id,
                source_occurred_at,
            })
            .await?;

        if let Some(outbound) = outbound {
            self.send_and_record(outbound).await?;
        }

        Ok(())
    }

    pub async fn handle_email_event(&self, event: EmailEvent) -> EventHandleResult {
        let delivery = DeliveryContext::default();
        match event {
            EmailEvent::VerificationEmailRequested(payload) => {
                Arc::new(self.clone())
                    .handle_verification_email_requested(delivery, payload)
                    .await
            }
        }
    }

    async fn insert_outbound_email(
        &self,
        new_email: NewOutboundEmail,
    ) -> Result<Option<outbound_email::Model>, EventHandleError> {
        if outbound_email::Entity::find()
            .filter(outbound_email::Column::DedupeKey.eq(&new_email.dedupe_key))
            .one(&self.db)
            .await
            .map_err(|e| EventHandleError::Transient(format!("outbound lookup failed: {e}")))?
            .is_some()
        {
            return Ok(None);
        }

        let now = Utc::now();
        let model = outbound_email::ActiveModel {
            id: Set(Uuid::new_v4()),
            dedupe_key: Set(new_email.dedupe_key),
            email_kind: Set(new_email.email_kind),
            recipient_user_id: Set(new_email.recipient_user_id),
            recipient_email: Set(new_email.recipient_email),
            provider_message_id: Set(None),
            provider_name: Set(Some(self.smtp_provider_name.clone())),
            template_key: Set(new_email.template_key),
            template_version: Set(new_email.template_version),
            subject: Set(new_email.subject),
            body_text: Set(new_email.body_text),
            body_html: Set(new_email.body_html),
            source_event_type: Set(new_email.source_event_type),
            source_event_id: Set(new_email.source_event_id),
            source_occurred_at: Set(new_email.source_occurred_at.into()),
            send_status: Set("pending".to_string()),
            last_error_code: Set(None),
            last_error_message: Set(None),
            next_attempt_after: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&self.db)
        .await
        .map_err(|e| EventHandleError::Transient(format!("outbound insert failed: {e}")))?;

        Ok(Some(model))
    }

    async fn send_and_record(&self, outbound: outbound_email::Model) -> EventHandleResult {
        let attempted_at = Utc::now();
        let attempt_number = self.next_attempt_number(outbound.id).await?;
        let send_result = self.send_email(&outbound).await;

        match send_result {
            Ok(()) => {
                email_delivery_attempt::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    outbound_email_id: Set(outbound.id),
                    attempt_number: Set(attempt_number),
                    provider_name: Set(self.smtp_provider_name.clone()),
                    provider_message_id: Set(None),
                    attempt_status: Set("submitted".to_string()),
                    failure_code: Set(None),
                    failure_message: Set(None),
                    attempted_at: Set(attempted_at.into()),
                    provider_response_snapshot: Set(Some(serde_json::json!({
                        "provider": self.smtp_provider_name.as_str(),
                    }))),
                }
                .insert(&self.db)
                .await
                .map_err(|e| EventHandleError::Transient(format!("attempt insert failed: {e}")))?;

                let mut active = outbound.into_active_model();
                active.provider_message_id = Set(None);
                active.provider_name = Set(Some(self.smtp_provider_name.clone()));
                active.send_status = Set("submitted".to_string());
                active.last_error_code = Set(None);
                active.last_error_message = Set(None);
                active.next_attempt_after = Set(None);
                active.updated_at = Set(Utc::now().into());
                active.update(&self.db).await.map_err(|e| {
                    EventHandleError::Transient(format!("outbound update failed: {e}"))
                })?;
                Ok(())
            }
            Err((status, code, message, retry_after, kind)) => {
                email_delivery_attempt::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    outbound_email_id: Set(outbound.id),
                    attempt_number: Set(attempt_number),
                    provider_name: Set(self.smtp_provider_name.clone()),
                    provider_message_id: Set(None),
                    attempt_status: Set(status.clone()),
                    failure_code: Set(code.clone()),
                    failure_message: Set(Some(message.clone())),
                    attempted_at: Set(attempted_at.into()),
                    provider_response_snapshot: Set(Some(serde_json::json!({
                        "error": message,
                    }))),
                }
                .insert(&self.db)
                .await
                .map_err(|e| EventHandleError::Transient(format!("attempt insert failed: {e}")))?;

                let mut active = outbound.into_active_model();
                active.provider_name = Set(Some(self.smtp_provider_name.clone()));
                active.send_status = Set(status);
                active.last_error_code = Set(code);
                active.last_error_message = Set(Some(message));
                active.next_attempt_after = Set(retry_after.map(Into::into));
                active.updated_at = Set(Utc::now().into());
                active.update(&self.db).await.map_err(|e| {
                    EventHandleError::Transient(format!("outbound update failed: {e}"))
                })?;
                Err(kind)
            }
        }
    }

    async fn next_attempt_number(&self, outbound_email_id: Uuid) -> Result<i32, EventHandleError> {
        let attempts = email_delivery_attempt::Entity::find()
            .filter(email_delivery_attempt::Column::OutboundEmailId.eq(outbound_email_id))
            .all(&self.db)
            .await
            .map_err(|e| EventHandleError::Transient(format!("attempt lookup failed: {e}")))?;

        Ok(attempts
            .iter()
            .map(|attempt| attempt.attempt_number)
            .max()
            .unwrap_or(0)
            + 1)
    }

    async fn send_email(
        &self,
        outbound: &outbound_email::Model,
    ) -> Result<
        (),
        (
            String,
            Option<String>,
            String,
            Option<DateTime<Utc>>,
            EventHandleError,
        ),
    > {
        self.smtp
            .send_email(outbound)
            .await
            .map_err(|err| match err {
                SmtpError::InvalidSender(e) => (
                    "failed".to_string(),
                    Some("invalid_sender".to_string()),
                    e,
                    None,
                    EventHandleError::Permanent("invalid sender mailbox".to_string()),
                ),
                SmtpError::InvalidRecipient(e) => (
                    "failed".to_string(),
                    Some("invalid_recipient".to_string()),
                    e,
                    None,
                    EventHandleError::Permanent("invalid recipient mailbox".to_string()),
                ),
                SmtpError::MessageBuild(e) => (
                    "failed".to_string(),
                    Some("message_build_failed".to_string()),
                    e,
                    None,
                    EventHandleError::Permanent("message build failed".to_string()),
                ),
                SmtpError::TransportConfig(e) => (
                    "retryable_failure".to_string(),
                    Some("smtp_configuration_error".to_string()),
                    e,
                    Some(Utc::now() + chrono::Duration::minutes(5)),
                    EventHandleError::Transient("smtp configuration error".to_string()),
                ),
                SmtpError::Send(e) => (
                    "retryable_failure".to_string(),
                    Some("smtp_send_failed".to_string()),
                    e,
                    Some(Utc::now() + chrono::Duration::minutes(5)),
                    EventHandleError::Transient("smtp send failed".to_string()),
                ),
            })
    }

    fn source_event_id<T: EventIdFallback>(
        &self,
        event_type: &str,
        payload: &T,
        delivery: &DeliveryContext,
    ) -> String {
        if let Some(message_id) = delivery.message_id.as_ref() {
            return message_id.clone();
        }

        if let Some(event_id) = delivery.headers.get("event_id") {
            return event_id.clone();
        }

        payload.fallback_event_id(event_type)
    }
}

struct NewOutboundEmail {
    dedupe_key: String,
    email_kind: String,
    recipient_user_id: Option<Uuid>,
    recipient_email: String,
    template_key: String,
    template_version: i32,
    subject: String,
    body_text: String,
    body_html: Option<String>,
    source_event_type: String,
    source_event_id: String,
    source_occurred_at: DateTime<Utc>,
}

trait EventIdFallback {
    fn fallback_event_id(&self, event_type: &str) -> String;
}

impl EventIdFallback for VerificationEmailRequested {
    fn fallback_event_id(&self, _event_type: &str) -> String {
        self.verification_token_id.clone()
    }
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid, EventHandleError> {
    Uuid::parse_str(value).map_err(|e| EventHandleError::Permanent(format!("invalid {field}: {e}")))
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, chrono::ParseError> {
    DateTime::parse_from_rfc3339(value).map(|value| value.with_timezone(&Utc))
}

impl RegistersAmqpRoutes for Handler {
    fn register(subscriber: RegisteredSubscriber<Self>) -> RegisteredSubscriber<Self> {
        subscriber.event(
            "identity.VerificationEmailRequested",
            route(Self::handle_verification_email_requested),
        )
    }
}
