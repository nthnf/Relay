use lapin::{
    BasicProperties, Confirmation, Connection, ConnectionProperties,
    options::{BasicPublishOptions, ConfirmSelectOptions, QueueDeclareOptions},
    types::{FieldTable, ShortString},
};
use relay_amqp::{
    AmqpSubscriber, DeliveryContext, EventHandleResult, RegisteredSubscriber, RegistersAmqpRoutes,
    route,
};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use testcontainers_modules::{
    rabbitmq::RabbitMq,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tokio::sync::oneshot;
use tokio::time::timeout;

type ReceivedMessage = (Option<String>, String);
type ResultSender = oneshot::Sender<ReceivedMessage>;
type SharedResultSender = Arc<Mutex<Option<ResultSender>>>;

#[derive(Clone)]
struct TestHandler {
    sender: SharedResultSender,
}

#[derive(Deserialize)]
struct TestPayload {
    message: String,
}

impl TestHandler {
    fn new(sender: ResultSender) -> Self {
        Self {
            sender: Arc::new(Mutex::new(Some(sender))),
        }
    }

    async fn handle_created(
        self: Arc<Self>,
        delivery: DeliveryContext,
        payload: TestPayload,
    ) -> EventHandleResult {
        if let Some(sender) = self.sender.lock().expect("sender mutex poisoned").take() {
            let _ = sender.send((delivery.message_id, payload.message));
        }

        Ok(())
    }
}

impl RegistersAmqpRoutes for TestHandler {
    fn register(subscriber: RegisteredSubscriber<Self>) -> RegisteredSubscriber<Self> {
        subscriber.event("test.EventCreated", route(Self::handle_created))
    }
}

#[tokio::test]
async fn consumes_event_from_rabbitmq_container() -> Result<(), Box<dyn std::error::Error>> {
    let rabbitmq = RabbitMq::default().start().await?;
    let rabbitmq_host = rabbitmq.get_host().await?;
    let rabbitmq_port = rabbitmq.get_host_port_ipv4(5672.tcp()).await?;
    let amqp_addr = format!("amqp://{rabbitmq_host}:{rabbitmq_port}/%2f");

    let queue_name = unique_name("relay-amqp.test.events");
    let (sender, receiver) = oneshot::channel();
    let subscriber_amqp_addr = amqp_addr.clone();
    let subscriber_queue_name = queue_name.clone();

    let run_task = tokio::spawn(async move {
        AmqpSubscriber::topic(
            "relay-amqp-test",
            subscriber_queue_name,
            "relay-amqp-test-consumer",
            "relay.events",
            "test.*",
        )
        .handle(TestHandler::new(sender))
        .run(&subscriber_amqp_addr)
        .await
    });

    wait_for_queue(&amqp_addr, &queue_name).await?;

    publish_test_event(
        &amqp_addr,
        "relay.events",
        "test.EventCreated",
        "message-123",
    )
    .await?;

    let received = timeout(Duration::from_secs(10), receiver).await??;
    assert_eq!(received.0, Some("message-123".to_string()));
    assert_eq!(received.1, "hello from rabbitmq".to_string());

    run_task.abort();
    let _ = run_task.await;

    Ok(())
}

async fn wait_for_queue(
    amqp_addr: &str,
    queue_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::connect(amqp_addr, ConnectionProperties::default()).await?;
    let deadline = Instant::now() + Duration::from_secs(10);

    loop {
        let channel = connection.create_channel().await?;
        let result = channel
            .queue_declare(
                queue_name.into(),
                QueueDeclareOptions {
                    passive: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await;

        match result {
            Ok(_) => return Ok(()),
            Err(error) if Instant::now() < deadline => {
                let _ = error;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(error) => return Err(Box::new(error)),
        }
    }
}

async fn publish_test_event(
    amqp_addr: &str,
    exchange: &str,
    routing_key: &str,
    message_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::connect(amqp_addr, ConnectionProperties::default()).await?;
    let channel = connection.create_channel().await?;
    let payload = serde_json::to_vec(&serde_json::json!({
        "message": "hello from rabbitmq",
    }))?;
    channel
        .confirm_select(ConfirmSelectOptions::default())
        .await?;

    let confirm = channel
        .basic_publish(
            exchange.into(),
            routing_key.into(),
            BasicPublishOptions::default(),
            payload.as_slice(),
            BasicProperties::default().with_message_id(ShortString::from(message_id.to_string())),
        )
        .await?;

    match confirm.await? {
        Confirmation::Ack(_) => Ok(()),
        Confirmation::Nack(_) => Err("broker returned nack for test publish".into()),
        Confirmation::NotRequested => Err("publisher confirms were not enabled".into()),
    }
}

fn unique_name(prefix: &str) -> String {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    format!("{prefix}.{suffix}")
}
