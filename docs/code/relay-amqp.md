# relay-amqp

## Purpose

`relay-amqp` is the shared AMQP consumer framework used by services like `workspace`, `friendship`, and `email`.

It exists to centralize the repetitive broker work:

- connect to RabbitMQ
- declare exchange and queue
- bind queue to routing key pattern
- consume deliveries
- match routing key to a registered handler
- parse JSON into a typed payload
- ack, reject, or requeue based on the handler result

This lets each service keep only:

- its own state
- its own business logic
- its own event-to-handler mapping

## The Main Idea

The framework is split into two layers.

1. `AmqpSubscriber`
   This is the runner. It knows AMQP topology and how to talk to RabbitMQ.

2. `Handler`
   This is service code. It owns state like `DatabaseConnection`, SMTP client, config, and domain logic.

So the service startup looks like this:

```rust
AmqpSubscriber::topic(
    "workspace",
    "workspace.events",
    "workspace-service",
    "relay.events",
    "identity.*",
)
.handle(amqp::AmqpHandler::new(db.clone()))
.run(&config.amqp_addr)
.await
```

The important part is that `main.rs` owns the AMQP topology, while the handler type owns the actual event handlers.

## The Flow

When a message arrives, this is what happens inside `relay-amqp`:

1. RabbitMQ delivers a raw `lapin::Delivery`
2. `relay-amqp` converts it into a smaller `DeliveryContext`
3. `relay-amqp` looks up the route by exact `routing_key`
4. If a route matches, it parses the JSON body into the requested payload type
5. It calls the registered handler method
6. It acks, rejects, or requeues depending on the returned `EventHandleError`

If no route matches, `relay-amqp` returns:

```rust
EventHandleError::Permanent("unknown routing key: ...")
```

That means the message is rejected and discarded rather than retried forever.

## The Public Pieces

The important public types in `crates/relay-amqp/src/lib.rs` are:

- `AmqpSubscriber`
- `RegisteredSubscriber<H>`
- `RegistersAmqpRoutes`
- `DeliveryContext`
- `EventHandleError`
- `EventHandleResult`
- `route(...)`

### `AmqpSubscriber`

This is the non-generic entry point.

It only knows:

- service name
- queue name
- consumer tag
- queue topology or topic topology

It does not know the handler type yet.

That is why `AmqpSubscriber` itself is not generic.

### `RegisteredSubscriber<H>`

Once you call `.handle(handler)`, the subscriber now knows the handler type, so it becomes:

```rust
RegisteredSubscriber<H>
```

`H` means: the concrete handler type for this service.

Examples:

- in `workspace`, `H` is `workspace::amqp::Handler`
- in `friendship`, `H` is `friendship::amqp::Handler`
- in `email`, `H` is `email::amqp::Handler`

This type stores:

- the AMQP config
- the handler instance, wrapped in `Arc<H>`
- the route table

## Why `Arc<H>` Exists

`Arc` means atomically reference-counted shared ownership.

In simpler words:

- there is one handler value
- many async route calls can share it safely
- cloning an `Arc` does not clone the whole database connection struct
- it only increments a small reference count

So this:

```rust
Arc<Handler>
```

does not mean:

- copying the database
- copying SMTP client state
- duplicating all handler fields

It means:

- multiple async tasks can hold a cheap shared pointer to the same handler state

## Why There Is a Trait

Each service needs to register different event routes.

For example:

- `workspace` listens to `identity.UserRegistered`, `identity.UserEmailVerified`, `identity.UserProfileUpdated`
- `email` listens to `identity.VerificationEmailRequested`

That is why the framework uses this trait:

```rust
pub trait RegistersAmqpRoutes {
    fn register(subscriber: RegisteredSubscriber<Self>) -> RegisteredSubscriber<Self>;
}
```

This means:

- the framework gives your handler type a `RegisteredSubscriber<Self>`
- your handler type adds its routes
- then returns the same subscriber

So route registration lives next to the handler type, not in `main.rs`.

Example from a service:

```rust
impl RegistersAmqpRoutes for Handler {
    fn register(subscriber: RegisteredSubscriber<Self>) -> RegisteredSubscriber<Self> {
        subscriber
            .event("identity.UserRegistered", route(Self::handle_user_registered))
            .event("identity.UserEmailVerified", route(Self::handle_user_email_verified))
    }
}
```

This is the main separation of concerns:

- `main.rs`: topology and startup
- `Handler`: state and business logic
- `impl RegistersAmqpRoutes`: event registration
- `relay-amqp`: delivery loop and dispatch

## Why `route(Self::handle_...)` Exists

This helper exists mostly to make registration readable.

Without it, registration looked like this:

```rust
.event("identity.UserRegistered", |handler, delivery, payload| async move {
    handler.handle_user_registered(delivery, payload).await
})
```

That works, but it is noisy.

With `route(...)`, registration becomes:

```rust
.event("identity.UserRegistered", route(Self::handle_user_registered))
```

The important part is not that `route(...)` does complicated work. Right now it is just a very small adapter.

The real reason this works is that all routed handler methods now use the same shape:

```rust
pub async fn handle_user_registered(
    self: Arc<Self>,
    delivery: DeliveryContext,
    payload: UserRegisteredPayload,
) -> EventHandleResult
```

Because every routed method has the same signature, the framework can accept them uniformly.

## Why Handler Methods Use `self: Arc<Self>`

This is the part that usually feels strange at first.

Why not just use `&self`?

Short answer:

- `&self` works fine for normal code
- but storing async method pointers in a generic library is harder with `&self`
- `Arc<Self>` is easier to move into async futures and store in the route table

So this shape:

```rust
self: Arc<Self>
```

helps avoid tricky lifetime problems.

You can think of it as:

- the framework owns a shared pointer to the handler
- each dispatch gets one cheap clone of that shared pointer
- the async method can safely keep using it until the future finishes

## The Generics, In Plain English

This is the part that usually looks scary:

```rust
pub fn event<T, F, Fut>(mut self, routing_key: impl Into<String>, handler: F) -> Self
where
    T: DeserializeOwned + Send + 'static,
    F: Fn(Arc<H>, DeliveryContext, T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = EventHandleResult> + Send + 'static,
```

Break it down one symbol at a time.

### `H`

`H` is the handler type.

Example:

```rust
RegisteredSubscriber<workspace::amqp::Handler>
```

So `H` answers: which service handler owns the state and methods?

### `T`

`T` is the payload type for one route.

Examples:

- `UserRegisteredPayload`
- `UserEmailVerifiedPayload`
- `VerificationEmailRequested`

So `T` answers: which JSON struct should this route deserialize into?

### `F`

`F` is the handler function itself.

In practice it is something like:

```rust
route(Self::handle_user_registered)
```

So `F` answers: what callable should run for this route?

### `Fut`

`Fut` is the future returned by the async handler.

Every `async fn` in Rust actually returns an anonymous future type.

So `Fut` answers: what async computation comes back when the handler is called?

## Why There Is a `Box<dyn ...>` Internally

Inside the route table, each route can have:

- a different payload type
- a different handler function
- a different future type

But a `HashMap` must store one uniform value type.

So the framework erases the concrete route type into one common shape:

```rust
type RouteHandler<H> = Arc<dyn Fn(Arc<H>, DeliveryContext, Vec<u8>) -> RouteFuture + Send + Sync>;
```

This means:

- every route is stored as “some callable that can handle bytes and return a boxed future”
- before storing it, `event(...)` wraps typed logic around it
- so the outside API stays typed, but the inside storage becomes uniform

This is a very common library pattern in Rust.

The public API is strongly typed.
The internal storage uses type erasure so different handlers can live in one collection.

## Why `DeliveryContext` Exists

The raw RabbitMQ delivery type is `lapin::Delivery`.

That object contains a lot of broker-specific detail.

Most application handlers do not need all of that.

So `relay-amqp` converts it into a smaller struct:

```rust
pub struct DeliveryContext {
    pub routing_key: String,
    pub message_id: Option<String>,
    pub correlation_id: Option<String>,
    pub headers: HashMap<String, String>,
}
```

That gives handlers the useful metadata without exposing ack/nack/reject mechanics.

The framework keeps broker-control behavior inside the framework.

## Error Model

Handlers return:

```rust
type EventHandleResult = Result<(), EventHandleError>
```

And the error is:

```rust
enum EventHandleError {
    Permanent(String),
    Transient(String),
}
```

Meaning:

- `Permanent`: do not retry, reject the message
- `Transient`: retry later, requeue the message

This is intentionally small. It gives every service the same retry vocabulary.

## Example: Email Service

The email service registers like this:

```rust
impl RegistersAmqpRoutes for Handler {
    fn register(subscriber: RegisteredSubscriber<Self>) -> RegisteredSubscriber<Self> {
        subscriber.event(
            "identity.VerificationEmailRequested",
            route(Self::handle_verification_email_requested),
        )
    }
}
```

And the handler method looks like this:

```rust
pub(super) async fn handle_verification_email_requested(
    self: Arc<Self>,
    delivery: DeliveryContext,
    payload: VerificationEmailRequested,
) -> EventHandleResult
```

So:

- `route(...)` gives the framework the method item
- `relay-amqp` deserializes JSON into `VerificationEmailRequested`
- `relay-amqp` passes `DeliveryContext`
- your method runs typed business logic

## Example: Workspace Service

The workspace service has multiple routes:

```rust
subscriber
    .event("identity.UserRegistered", route(Self::handle_user_registered))
    .event("identity.UserEmailVerified", route(Self::handle_user_email_verified))
    .event("identity.UserProfileUpdated", route(Self::handle_user_profile_updated))
```

That means one handler type can register many event names, each with a different payload type.

## Test Coverage

There are now two kinds of tests for this crate.

### Fast integration test

`crates/relay-amqp/tests/subscriber_routes.rs`

This tests:

- `route(Self::handle_...)`
- typed payload dispatch
- parse failure behavior
- unknown routing key behavior

It does not need RabbitMQ.

### Real RabbitMQ integration test

`crates/relay-amqp/tests/rabbitmq_flow.rs`

This test uses a RabbitMQ testcontainer and verifies:

- subscriber startup against a real broker
- event publish
- event consume
- delivery of `message_id` through `DeliveryContext`
- typed payload handling through the real AMQP loop

## Practical Mental Model

If the generic types still feel abstract, use this mental model:

1. `AmqpSubscriber` is the machine.
2. `Handler` is your service brain.
3. `RegistersAmqpRoutes` is the wiring step.
4. `event(...)` says which event name maps to which handler method.
5. `route(...)` lets method items fit the framework cleanly.
6. `Arc` is just the shared pointer that lets async code keep using the handler safely.
7. Internal `Box<dyn ...>` storage is only there because one route table must store many different concrete handler shapes.

## If You Ignore Most of the Generics

You can still use the crate effectively by remembering only this:

```rust
impl RegistersAmqpRoutes for Handler {
    fn register(subscriber: RegisteredSubscriber<Self>) -> RegisteredSubscriber<Self> {
        subscriber.event("some.Event", route(Self::handle_some_event))
    }
}
```

And handler methods should look like:

```rust
pub async fn handle_some_event(
    self: Arc<Self>,
    delivery: DeliveryContext,
    payload: SomePayload,
) -> EventHandleResult
```

Everything else is the framework taking care of the hard part.
