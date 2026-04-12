use crate::entity::outbound_email;
use lettre::message::{Mailbox, Message, MultiPart, SinglePart, header::ContentType};
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};

#[derive(Clone, Debug)]
pub struct SmtpClient {
    smtp_url: String,
    sender_email: String,
    sender_name: String,
}

#[derive(Debug)]
pub enum SmtpError {
    InvalidSender(String),
    InvalidRecipient(String),
    MessageBuild(String),
    TransportConfig(String),
    Send(String),
}

impl SmtpClient {
    pub fn new(smtp_url: String, sender_email: String, sender_name: String) -> Self {
        Self {
            smtp_url,
            sender_email,
            sender_name,
        }
    }

    pub async fn send_email(&self, outbound: &outbound_email::Model) -> Result<(), SmtpError> {
        let sender = if self.sender_name.is_empty() {
            self.sender_email.clone()
        } else {
            format!("{} <{}>", self.sender_name, self.sender_email)
        };

        let from = sender
            .parse::<Mailbox>()
            .map_err(|e| SmtpError::InvalidSender(e.to_string()))?;

        let to = outbound
            .recipient_email
            .parse::<Mailbox>()
            .map_err(|e| SmtpError::InvalidRecipient(e.to_string()))?;

        let message = build_message(
            from,
            to,
            &outbound.subject,
            &outbound.body_text,
            outbound.body_html.as_deref(),
        )?;

        let mailer = AsyncSmtpTransport::<Tokio1Executor>::from_url(&self.smtp_url)
            .map_err(|e| SmtpError::TransportConfig(e.to_string()))?
            .build();

        mailer
            .send(message)
            .await
            .map_err(|e| SmtpError::Send(e.to_string()))?;

        Ok(())
    }
}

fn build_message(
    from: Mailbox,
    to: Mailbox,
    subject: &str,
    body_text: &str,
    body_html: Option<&str>,
) -> Result<Message, SmtpError> {
    let builder = Message::builder().from(from).to(to).subject(subject);

    if let Some(body_html) = body_html {
        builder
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(body_text.to_string()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(body_html.to_string()),
                    ),
            )
            .map_err(|e| SmtpError::MessageBuild(e.to_string()))
    } else {
        builder
            .body(body_text.to_string())
            .map_err(|e| SmtpError::MessageBuild(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_multipart_when_html_present() {
        let from = "Relay <relay@example.com>".parse().unwrap();
        let to = "User <user@example.com>".parse().unwrap();

        let message = build_message(from, to, "Subject", "plain", Some("<p>html</p>")).unwrap();
        let formatted = String::from_utf8(message.formatted()).unwrap();

        assert!(formatted.contains("multipart/alternative"));
        assert!(formatted.contains("text/plain"));
        assert!(formatted.contains("text/html"));
        assert!(formatted.contains("plain"));
        assert!(formatted.contains("<p>html</p>"));
    }

    #[test]
    fn builds_plain_body_when_html_missing() {
        let from = "Relay <relay@example.com>".parse().unwrap();
        let to = "User <user@example.com>".parse().unwrap();

        let message = build_message(from, to, "Subject", "plain", None).unwrap();
        let formatted = String::from_utf8(message.formatted()).unwrap();

        assert!(!formatted.contains("multipart/alternative"));
        assert!(formatted.contains("plain"));
    }
}
