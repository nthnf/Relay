pub mod events;
pub mod handler;

use crate::amqp::handler::{HandleError, Handler};
use lapin::message::DeliveryResult;
use lapin::options::{BasicAckOptions, BasicNackOptions, BasicQosOptions, BasicRejectOptions};
use lapin::{Connection, ConnectionProperties, options::BasicConsumeOptions, types::FieldTable};
use tracing::{error, info, warn};
use std::sync::Arc;

pub async fn run(handler: Arc<Handler>, amqp_addr: String) -> Result<(), Box<dyn std::error::Error>> {
    let amqp = Connection::connect(&amqp_addr, ConnectionProperties::default()).await?;
    let channel = amqp.create_channel().await?;

    channel.basic_qos(16, BasicQosOptions::default()).await?;

    let consumer = channel
        .basic_consume(
            "email.events".into(),
            "email-service".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    info!(exchange = "relay.events", "email service started");

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
                        Err(HandleError::Permanent(e)) => {
                            warn!(error = %e, "permanent error handling message, discarding");
                            if let Err(reject_error) =
                                delivery.reject(BasicRejectOptions { requeue: false }).await
                            {
                                error!(error = %reject_error, "failed to reject message");
                            }
                        }
                        Err(HandleError::Transient(e)) => {
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
