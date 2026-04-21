use relay_amqp::{
    AmqpSubscriber, DeliveryContext, EventHandleError, EventHandleResult, RegisteredSubscriber,
    RegistersAmqpRoutes, route,
};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

type RecordedCall = (String, Option<String>, String);
type RecordedCalls = Arc<Mutex<Vec<RecordedCall>>>;

#[derive(Clone, Default)]
struct TestHandler {
    calls: RecordedCalls,
}

#[derive(Deserialize)]
struct TestPayload {
    value: String,
}

impl TestHandler {
    async fn handle_value(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: TestPayload,
    ) -> EventHandleResult {
        self.calls.lock().expect("calls mutex poisoned").push((
            delivery.routing_key,
            delivery.message_id,
            payload.value,
        ));
        Ok(())
    }
}

impl RegistersAmqpRoutes for TestHandler {
    fn register(subscriber: RegisteredSubscriber<Self>) -> RegisteredSubscriber<Self> {
        subscriber.event("test.ValueCreated", route(Self::handle_value))
    }
}

#[tokio::test]
async fn route_dispatches_method_item_with_typed_payload() -> Result<(), Box<dyn std::error::Error>>
{
    let handler = TestHandler::default();
    let subscriber =
        AmqpSubscriber::queue("test", "test.events", "test-service").handle(handler.clone());

    subscriber
        .dispatch_for_test(
            DeliveryContext {
                routing_key: "test.ValueCreated".to_string(),
                message_id: Some("message-123".to_string()),
                ..Default::default()
            },
            serde_json::to_vec(&serde_json::json!({
                "value": "hello",
            }))?,
        )
        .await?;

    let calls = handler.calls.lock().expect("calls mutex poisoned");
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0],
        (
            "test.ValueCreated".to_string(),
            Some("message-123".to_string()),
            "hello".to_string()
        )
    );

    Ok(())
}

#[tokio::test]
async fn dispatch_reports_unknown_routing_key_as_permanent_error() {
    let subscriber =
        AmqpSubscriber::queue("test", "test.events", "test-service").handle(TestHandler::default());

    let error = subscriber
        .dispatch_for_test(
            DeliveryContext {
                routing_key: "test.Unknown".to_string(),
                ..Default::default()
            },
            b"{}".to_vec(),
        )
        .await
        .expect_err("unknown route should fail");

    match error {
        EventHandleError::Permanent(message) => {
            assert_eq!(message, "unknown routing key: test.Unknown")
        }
        EventHandleError::Transient(message) => {
            panic!("expected permanent error, got transient: {message}")
        }
    }
}

#[tokio::test]
async fn dispatch_reports_parse_errors_as_permanent_errors() {
    let subscriber =
        AmqpSubscriber::queue("test", "test.events", "test-service").handle(TestHandler::default());

    let error = subscriber
        .dispatch_for_test(
            DeliveryContext {
                routing_key: "test.ValueCreated".to_string(),
                ..Default::default()
            },
            b"not-json".to_vec(),
        )
        .await
        .expect_err("invalid payload should fail");

    match error {
        EventHandleError::Permanent(message) => {
            assert!(message.starts_with("failed to parse event:"));
        }
        EventHandleError::Transient(message) => {
            panic!("expected permanent error, got transient: {message}")
        }
    }
}
