use lapin::message::DeliveryResult;
use lapin::options::{
    BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicQosOptions, BasicRejectOptions,
    ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
};
use lapin::types::{AMQPValue, FieldTable};
use lapin::{Connection, ConnectionProperties, ExchangeKind};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

const CONNECT_RETRY_DELAY: Duration = Duration::from_secs(2);
const CONNECT_MAX_RETRIES: usize = 30;

#[derive(Debug)]
pub enum EventHandleError {
    Permanent(String),
    Transient(String),
}

pub type EventHandleResult = Result<(), EventHandleError>;

impl std::fmt::Display for EventHandleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Permanent(message) | Self::Transient(message) => f.write_str(message),
        }
    }
}

impl Error for EventHandleError {}

impl From<serde_json::Error> for EventHandleError {
    fn from(error: serde_json::Error) -> Self {
        Self::Permanent(format!("failed to parse event: {error}"))
    }
}

#[derive(Clone, Debug, Default)]
pub struct DeliveryContext {
    pub routing_key: String,
    pub message_id: Option<String>,
    pub correlation_id: Option<String>,
    pub headers: HashMap<String, String>,
}

impl DeliveryContext {
    fn from_delivery(delivery: &lapin::message::Delivery) -> Self {
        let headers = delivery
            .properties
            .headers()
            .as_ref()
            .map_or_else(HashMap::new, header_strings);

        Self {
            routing_key: delivery.routing_key.to_string(),
            message_id: delivery
                .properties
                .message_id()
                .as_ref()
                .map(ToString::to_string),
            correlation_id: delivery
                .properties
                .correlation_id()
                .as_ref()
                .map(ToString::to_string),
            headers,
        }
    }
}

type RouteFuture = Pin<Box<dyn Future<Output = EventHandleResult> + Send>>;
type RouteHandler<H> = Arc<dyn Fn(Arc<H>, DeliveryContext, Vec<u8>) -> RouteFuture + Send + Sync>;

#[derive(Clone, Debug)]
pub enum ConsumerTopology {
    Topic {
        exchange: String,
        binding_key: String,
    },
    Queue,
}

#[derive(Clone, Debug)]
struct ConsumerConfig {
    service_name: String,
    queue: String,
    consumer_tag: String,
    topology: ConsumerTopology,
    prefetch: u16,
}

impl ConsumerConfig {
    fn topic(
        service_name: impl Into<String>,
        queue: impl Into<String>,
        consumer_tag: impl Into<String>,
        exchange: impl Into<String>,
        binding_key: impl Into<String>,
    ) -> Self {
        Self {
            service_name: service_name.into(),
            queue: queue.into(),
            consumer_tag: consumer_tag.into(),
            topology: ConsumerTopology::Topic {
                exchange: exchange.into(),
                binding_key: binding_key.into(),
            },
            prefetch: 16,
        }
    }

    fn queue(
        service_name: impl Into<String>,
        queue: impl Into<String>,
        consumer_tag: impl Into<String>,
    ) -> Self {
        Self {
            service_name: service_name.into(),
            queue: queue.into(),
            consumer_tag: consumer_tag.into(),
            topology: ConsumerTopology::Queue,
            prefetch: 16,
        }
    }
}

pub struct AmqpSubscriber {
    config: ConsumerConfig,
}

pub struct RegisteredSubscriber<H> {
    config: ConsumerConfig,
    handler: Arc<H>,
    routes: HashMap<String, RouteHandler<H>>,
}

pub trait RegistersAmqpRoutes: Sized + Send + Sync + 'static {
    fn register(subscriber: RegisteredSubscriber<Self>) -> RegisteredSubscriber<Self>;
}

pub fn route<H, T, F, Fut>(handler: F) -> F
where
    H: Send + Sync + 'static,
    T: DeserializeOwned + Send + 'static,
    F: Fn(Arc<H>, DeliveryContext, T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = EventHandleResult> + Send + 'static,
{
    handler
}

impl AmqpSubscriber {
    pub fn topic(
        service_name: impl Into<String>,
        queue: impl Into<String>,
        consumer_tag: impl Into<String>,
        exchange: impl Into<String>,
        binding_key: impl Into<String>,
    ) -> Self {
        Self {
            config: ConsumerConfig::topic(service_name, queue, consumer_tag, exchange, binding_key),
        }
    }

    pub fn queue(
        service_name: impl Into<String>,
        queue: impl Into<String>,
        consumer_tag: impl Into<String>,
    ) -> Self {
        Self {
            config: ConsumerConfig::queue(service_name, queue, consumer_tag),
        }
    }

    pub fn handle<H>(self, handler: H) -> RegisteredSubscriber<H>
    where
        H: RegistersAmqpRoutes,
    {
        H::register(RegisteredSubscriber {
            config: self.config,
            handler: Arc::new(handler),
            routes: HashMap::new(),
        })
    }
}

impl<H> RegisteredSubscriber<H>
where
    H: Send + Sync + 'static,
{
    pub fn event<T, F, Fut>(mut self, routing_key: impl Into<String>, handler: F) -> Self
    where
        T: DeserializeOwned + Send + 'static,
        F: Fn(Arc<H>, DeliveryContext, T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = EventHandleResult> + Send + 'static,
    {
        let handler = Arc::new(handler);
        let route_handler: RouteHandler<H> = Arc::new(move |state, delivery, body| {
            let handler = handler.clone();
            Box::pin(async move {
                let payload: T = serde_json::from_slice(&body).map_err(EventHandleError::from)?;
                handler(state, delivery, payload).await
            })
        });

        self.routes.insert(routing_key.into(), route_handler);
        self
    }

    async fn dispatch(
        routes: &HashMap<String, RouteHandler<H>>,
        handler: Arc<H>,
        context: DeliveryContext,
        body: Vec<u8>,
    ) -> EventHandleResult {
        if let Some(route) = routes.get(context.routing_key.as_str()) {
            route(handler, context, body).await
        } else {
            Err(EventHandleError::Permanent(format!(
                "unknown routing key: {}",
                context.routing_key
            )))
        }
    }

    async fn handle_delivery(
        routes: &HashMap<String, RouteHandler<H>>,
        handler: Arc<H>,
        delivery: &lapin::message::Delivery,
    ) -> EventHandleResult {
        Self::dispatch(
            routes,
            handler,
            DeliveryContext::from_delivery(delivery),
            delivery.data.clone(),
        )
        .await
    }

    #[doc(hidden)]
    pub async fn dispatch_for_test(
        &self,
        context: DeliveryContext,
        body: impl Into<Vec<u8>>,
    ) -> EventHandleResult {
        Self::dispatch(&self.routes, self.handler.clone(), context, body.into()).await
    }

    pub async fn run(self, amqp_addr: impl AsRef<str>) -> Result<(), Box<dyn Error + Send + Sync>> {
        let amqp = connect_with_retry(amqp_addr.as_ref()).await?;
        let channel = amqp.create_channel().await?;

        match &self.config.topology {
            ConsumerTopology::Topic {
                exchange,
                binding_key,
            } => {
                channel
                    .exchange_declare(
                        exchange.clone().into(),
                        ExchangeKind::Topic,
                        ExchangeDeclareOptions {
                            durable: true,
                            ..Default::default()
                        },
                        FieldTable::default(),
                    )
                    .await?;

                channel
                    .queue_declare(
                        self.config.queue.clone().into(),
                        QueueDeclareOptions {
                            durable: true,
                            ..Default::default()
                        },
                        FieldTable::default(),
                    )
                    .await?;

                channel
                    .queue_bind(
                        self.config.queue.clone().into(),
                        exchange.clone().into(),
                        binding_key.clone().into(),
                        QueueBindOptions::default(),
                        FieldTable::default(),
                    )
                    .await?;
            }
            ConsumerTopology::Queue => {
                channel
                    .queue_declare(
                        self.config.queue.clone().into(),
                        QueueDeclareOptions {
                            durable: true,
                            ..Default::default()
                        },
                        FieldTable::default(),
                    )
                    .await?;
            }
        }

        channel
            .basic_qos(self.config.prefetch, BasicQosOptions::default())
            .await?;

        let consumer = channel
            .basic_consume(
                self.config.queue.clone().into(),
                self.config.consumer_tag.clone().into(),
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let service_name = self.config.service_name.clone();
        let queue = self.config.queue.clone();
        let routes = Arc::new(self.routes);
        let handler = self.handler.clone();
        info!(service = %service_name, queue = %queue, "amqp consumer started");

        consumer.set_delegate(move |delivery: DeliveryResult| {
            let routes = routes.clone();
            let handler = handler.clone();
            let service_name = service_name.clone();

            async move {
                match delivery {
                    Ok(Some(delivery)) => {
                        let result = Self::handle_delivery(routes.as_ref(), handler, &delivery).await;

                        match result {
                            Ok(()) => {
                                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                    error!(error = %e, service = %service_name, "failed to ack message");
                                }
                            }
                            Err(EventHandleError::Permanent(message)) => {
                                warn!(error = %message, service = %service_name, "permanent error handling message, discarding");
                                if let Err(reject_error) = delivery
                                    .reject(BasicRejectOptions { requeue: false })
                                    .await
                                {
                                    error!(error = %reject_error, service = %service_name, "failed to reject message");
                                }
                            }
                            Err(EventHandleError::Transient(message)) => {
                                warn!(error = %message, service = %service_name, "transient error handling message, requeuing");
                                if let Err(nack_error) = delivery
                                    .nack(BasicNackOptions {
                                        requeue: true,
                                        multiple: false,
                                    })
                                    .await
                                {
                                    error!(error = %nack_error, service = %service_name, "failed to nack message");
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        info!(service = %service_name, "consumer stream ended");
                    }
                    Err(e) => {
                        error!(error = %e, service = %service_name, "error receiving message");
                    }
                }
            }
        });

        tokio::signal::ctrl_c().await?;

        Ok(())
    }
}

async fn connect_with_retry(amqp_addr: &str) -> Result<Connection, Box<dyn Error + Send + Sync>> {
    let mut last_error = None;

    for attempt in 1..=CONNECT_MAX_RETRIES {
        match Connection::connect(amqp_addr, ConnectionProperties::default()).await {
            Ok(connection) => return Ok(connection),
            Err(error) => {
                warn!(attempt, error = %error, "amqp connection failed; retrying");
                last_error = Some(error);
                sleep(CONNECT_RETRY_DELAY).await;
            }
        }
    }

    Err(Box::new(
        last_error.expect("amqp connection should have been attempted"),
    ))
}

fn header_strings(headers: &FieldTable) -> HashMap<String, String> {
    let mut result = HashMap::new();

    for (key, value) in headers.inner() {
        let value = match value {
            AMQPValue::LongString(value) => Some(value.to_string()),
            AMQPValue::ShortString(value) => Some(value.to_string()),
            _ => None,
        };

        if let Some(value) = value {
            result.insert(key.to_string(), value);
        }
    }

    result
}
