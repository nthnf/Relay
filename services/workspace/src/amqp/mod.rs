pub mod handler;

pub use handler::Handler as AmqpHandler;

use handler::{AmqpError, Handler};
use lapin::message::DeliveryResult;
use lapin::options::{BasicAckOptions, BasicNackOptions, BasicQosOptions, BasicRejectOptions};
use lapin::{
    Connection, ConnectionProperties, ExchangeKind,
    options::{BasicConsumeOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions},
    types::FieldTable,
};
use std::sync::Arc;
use tracing::{error, info, warn};

const EXCHANGE: &str = "relay.events";
const QUEUE: &str = "workspace.events";
const BINDING_KEY: &str = "identity.*";

pub async fn run(
    handler: Arc<Handler>,
    amqp_addr: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let amqp = Connection::connect(&amqp_addr, ConnectionProperties::default()).await?;
    let channel = amqp.create_channel().await?;

    channel
        .exchange_declare(
            EXCHANGE.into(),
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
            QUEUE.into(),
            QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    channel
        .queue_bind(
            QUEUE.into(),
            EXCHANGE.into(),
            BINDING_KEY.into(),
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    channel.basic_qos(16, BasicQosOptions::default()).await?;

    let consumer = channel
        .basic_consume(
            QUEUE.into(),
            "friendship-service".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    info!(exchange = "relay.events", "friendship amqp started");

    consumer.set_delegate(move |delivery: DeliveryResult| {
        let handler = handler.clone();

        async move {
            match delivery {
                Ok(Some(delivery)) => {
                    let result = handler.handle_delivery(&delivery).await;

                    match result {
                        Ok(()) => {
                            if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                error!(error = %e, "failed to ack message");
                            }
                        }
                        Err(AmqpError::Permanent(e)) => {
                            warn!(error = %e, "permanent error handling message, discarding");
                            if let Err(reject_error) =
                                delivery.reject(BasicRejectOptions { requeue: false }).await
                            {
                                error!(error = %reject_error, "failed to reject message");
                            }
                        }
                        Err(AmqpError::Transient(e)) => {
                            warn!(error = %e, "transient error handling message, requeuing");
                            if let Err(nack_error) = delivery
                                .nack(BasicNackOptions {
                                    requeue: true,
                                    multiple: false,
                                })
                                .await
                            {
                                error!(error = %nack_error, "failed to nack message");
                            }
                        }
                    }
                }
                Ok(None) => {
                    info!("consumer stream ended");
                }
                Err(e) => {
                    error!(error = %e, "error receiving message");
                }
            }
        }
    });

    tokio::signal::ctrl_c().await?;

    Ok(())
}
