# Relay

Relay is a realtime team communication app built as a set of small backend services. It supports user accounts, email verification, workspaces, channels, direct messages, friendships, invite links, message history, unread state, and realtime message delivery.

The repository is intentionally split between product-facing code and infrastructure-facing code:

- `web/` is the SvelteKit browser app and backend-for-frontend.
- `services/` contains Rust gRPC services for the core domains.
- `workers/outbox/` publishes persisted domain events to RabbitMQ.
- `proto/` is the contract boundary shared by backend services and generated web clients.
- `deployment/k8s/` contains the local Kubernetes stack and optional Envoy Gateway routing.

## What The App Is

Relay is a chat and collaboration application with two main conversation surfaces:

- Workspaces, which contain members, invite links, and ordered channels.
- Direct messages, which are backed by one-to-one conversations between users.

Users register with email/password, verify email, maintain a profile, add or block friends, create workspaces, invite members, create channels, and send/edit/delete messages. The web app does not call backend gRPC services directly from the browser. Instead, SvelteKit acts as a BFF that exposes browser-safe HTTP routes and calls the internal gRPC services from server-only code.

## Architecture

```text
Browser
  |
  | HTTP pages/actions/API
  v
SvelteKit web BFF
  |
  | server-side gRPC clients
  v
Envoy Gateway, optional locally but production-shaped
  |
  | gRPC / WebSocket routing and auth checks
  v
Rust backend services
  |
  | service-owned Postgres databases
  | RabbitMQ domain events
  | Redis realtime presence/session state
  v
Outbox workers and realtime delivery
```

The backend uses service boundaries rather than a single shared database. Each DB-owning service has its own Postgres instance in the Kubernetes manifests. Cross-service contracts are protobuf/gRPC, and asynchronous integration uses RabbitMQ events published through an outbox worker.

## Backend Services

- `identity`: registration, password authentication, refresh/revoke sessions, email verification, profile lookup/update, and token validation for Envoy external auth.
- `workspace`: workspace creation, metadata, membership, channel management, invitations, invite links, and channel authorization checks.
- `chat`: conversation creation, message create/edit/delete, message listing, unread/read state, and calls to workspace/realtime where needed.
- `friendship`: friend requests, accepts/rejects, friend removal, blocking, unblocking, and friend/block lists.
- `bootstrap`: composed read models for loading the app shell, workspace views, and DM list without forcing the web app to call many services.
- `email`: consumes email-related work and sends verification email through SMTP configuration.
- `realtime`: gRPC delivery API plus WebSocket support for realtime message fanout and presence.

Shared crates:

- `crates/relay-proto`: generated Rust protobuf/gRPC bindings.
- `crates/relay-amqp`: RabbitMQ/outbox messaging helpers.
- `crates/relay-types`: shared domain types.

## Web BFF

`web/` is a SvelteKit app. Its main job is to keep the browser contract stable and hide internal gRPC details.

Current BFF shape:

- Auth endpoints live under `/auth/*` and set/rotate httpOnly cookies.
- App/resource endpoints live under `/api/*`.
- Server-only gRPC clients live in `.server.ts` code.
- Browser routes consume HTTP/page data, not gRPC service names.

Examples from the current BFF boundary:

- `GET /api/user-app` calls `BootstrapService.GetAppBootstrap`.
- `GET /api/workspaces/:workspaceId` calls `BootstrapService.GetWorkspaceBootstrap`.
- `GET /api/dm-threads` calls `BootstrapService.GetDmBootstrap`.
- `POST /api/messages` calls `ChatService.CreateMessage`.

## Infrastructure

The local infrastructure is Kubernetes-first and lives in `deployment/k8s/`.

Core local stack:

- Namespace: `relay`.
- App deployments: `identity`, `bootstrap`, `chat`, `email`, `friendship`, `workspace`, `realtime`.
- Outbox deployments: one `relay/outbox:local` worker per event-producing service.
- Databases: separate `postgres:18-alpine` StatefulSets for each DB-owning service.
- Broker: `rabbitmq:4.3-management-alpine` with local definitions.
- Realtime state: `redis:8.6-alpine`.
- Migrations: Kubernetes Jobs run each service migration binary before normal app use.

Optional gateway stack:

- Envoy Gateway routes gRPC traffic by authority, for example `identity.local`, `bootstrap.local`, `chat.local`, `friendship.local`, and `workspace.local`.
- Protected backend routes use `identity` as Envoy gRPC external auth.
- Realtime WebSocket traffic is routed at `/ws` and protected by the same auth path.
- MetalLB can provide local `LoadBalancer` IPs for kind when desired.

The deployment model is deliberately close to a production shape: services are containerized separately, data ownership is isolated, secrets/config are injected through Kubernetes `Secret` and `ConfigMap`, and ingress/auth concerns are handled at the gateway boundary.

## Event Flow

Relay uses a transactional outbox pattern for durable domain events.

1. A service writes domain state and an outbox row in its own Postgres database.
2. The matching outbox worker polls and claims rows for that service.
3. The worker publishes events to RabbitMQ exchange `relay.events`.
4. Consumers react asynchronously, such as email sending, bootstrap projection updates, or realtime notification paths.

This keeps service writes local while still allowing other parts of the system to react to changes.

## Realtime Flow

Realtime delivery is split between durable chat state and active client sessions.

1. `chat` validates and stores a message.
2. `chat` calls `realtime` for immediate delivery where applicable.
3. `realtime` tracks connected WebSocket sessions and presence state with Redis.
4. Connected recipients receive events such as `message_created`, `message_edited`, and `message_deleted`.

## Local Kubernetes

Build local service images from the repository root:

```sh
docker build -f services/identity/Dockerfile -t relay/identity:local .
docker build -f services/bootstrap/Dockerfile -t relay/bootstrap:local .
docker build -f services/chat/Dockerfile -t relay/chat:local .
docker build -f services/email/Dockerfile -t relay/email:local .
docker build -f services/friendship/Dockerfile -t relay/friendship:local .
docker build -f services/workspace/Dockerfile -t relay/workspace:local .
docker build -f services/realtime/Dockerfile -t relay/realtime:local .
docker build -f workers/outbox/Dockerfile -t relay/outbox:local .
```

Create and load a kind cluster:

```sh
kind create cluster --config deployment/k8s/local-kind/kind-config.yaml
kind load docker-image relay/identity:local --name relay
kind load docker-image relay/bootstrap:local --name relay
kind load docker-image relay/chat:local --name relay
kind load docker-image relay/email:local --name relay
kind load docker-image relay/friendship:local --name relay
kind load docker-image relay/workspace:local --name relay
kind load docker-image relay/realtime:local --name relay
kind load docker-image relay/outbox:local --name relay
kubectl apply -k deployment/k8s/local-kind
```

Install the optional Envoy Gateway overlay:

```sh
helm install eg oci://docker.io/envoyproxy/gateway-helm --version 1.7.2 -n envoy-gateway-system --create-namespace
kubectl wait --timeout=5m -n envoy-gateway-system deployment/envoy-gateway --for=condition=Available
kubectl apply -k deployment/k8s/local-kind-envoy
```

Run the Envoy-backed chat smoke test:

```sh
./e2e/chat-envoy.sh
```

## Development Notes

Generate web protobuf clients from `web/`:

```sh
bun run proto:gen
```

Run the web app from `web/`:

```sh
bun install
bun run dev
```

Useful checks:

```sh
cargo test
cd web && bun run check
```

## Configuration

Important runtime variables include:

- `DATABASE_URL`: service-owned Postgres connection string.
- `AMQP_ADDR`: RabbitMQ connection string.
- `TOKEN_SECRET`: identity token signing secret.
- `WORKSPACE_SERVICE_URL`: chat-to-workspace gRPC URL.
- `REALTIME_SERVICE_URL`: chat-to-realtime gRPC URL.
- `REDIS_URL`: realtime Redis connection string.
- `SMTP_URL`, `SMTP_PROVIDER_NAME`, `SENDER_EMAIL`, `SENDER_NAME`: email delivery settings.
- `GRPC_TARGET` and `*_GRPC_AUTHORITY`: web BFF settings for Envoy-backed gRPC calls.

Local Kubernetes manifests include development defaults only. Real credentials should be created out of band as Kubernetes Secrets.
