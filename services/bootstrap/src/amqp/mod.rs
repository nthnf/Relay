mod chat;
mod friendship;
mod handler;
mod identity;
mod workspace;

pub use handler::Handler as AmqpHandler;

use relay_amqp::{RegisteredSubscriber, RegistersAmqpRoutes};

impl RegistersAmqpRoutes for AmqpHandler {
    fn register(subscriber: RegisteredSubscriber<Self>) -> RegisteredSubscriber<Self> {
        let subscriber = identity::register(subscriber);
        let subscriber = friendship::register(subscriber);
        let subscriber = workspace::register(subscriber);
        chat::register(subscriber)
    }
}
