# Relay Web BFF

The SvelteKit app is the browser-facing backend-for-frontend (BFF). Browser code should call resource/use-case HTTP endpoints, not gRPC service and method names.

Example:

```http
GET /api/user-app
```

`/api/user-app` calls `BootstrapService.GetAppBootstrap` on the SvelteKit server and returns JSON to the browser.

## BFF Boundary

- Do expose stable browser endpoints like `/api/user-app`, `/api/workspaces/:id`, `/api/dm-threads`, `/api/messages`.
- Keep auth endpoints outside the `/api` prefix, for example `/auth/login`, `/auth/register`, `/auth/refresh`.
- Do not expose generic routes like `/api/grpc/:service/:method`.
- Keep gRPC clients server-only in `.server.ts` files.
- Prefer `bootstrap` for composed read models.
- Let Envoy validate access tokens for protected backend gRPC routes and inject `x-user-id`.

For local Envoy-backed development, point server-side gRPC clients at the Envoy LoadBalancer and set authority per service:

```sh
GRPC_TARGET=172.19.255.200:8080
BOOTSTRAP_GRPC_AUTHORITY=bootstrap.local
CHAT_GRPC_AUTHORITY=chat.local
WORKSPACE_GRPC_AUTHORITY=workspace.local
AUTHORIZATION_HEADER_NAME=authorization
ACCESS_TOKEN_COOKIE_NAME=access_token
```

`GRPC_TARGET` or a service-specific target is required. This is intentional: the BFF should call Envoy on `:8080`, not individual backend Services on `:50051`. When running inside a cluster, use the Envoy Service DNS name as `GRPC_TARGET` and keep the same authority values.

## Current BFF Routes

Auth routes:

- `POST /auth/register` -> `identity.RegisterUser`
- `POST /auth/login` -> `identity.AuthenticatePassword`, then sets httpOnly auth cookies
- `POST /auth/refresh` -> `identity.RefreshSession`, then rotates httpOnly auth cookies
- `POST /auth/logout` -> `identity.RevokeSession`, then clears auth cookies
- `POST /auth/verify-email` -> `identity.RedeemEmailVerificationToken`, then sets httpOnly auth cookies
- `POST /auth/resend-verification` -> `identity.ResendVerificationEmail`

Application routes:

- `GET /api/user-app` -> `bootstrap.GetAppBootstrap`
- `GET /api/workspaces/:workspaceId` -> `bootstrap.GetWorkspaceBootstrap`
- `GET /api/dm-threads` -> `bootstrap.GetDmBootstrap`
- `POST /api/conversations` -> `chat.CreateConversation`
- `POST /api/messages` -> `chat.CreateMessage`
- `GET /api/messages?conversationId=...` -> `chat.ListConversationMessages`
- `PATCH /api/messages/:messageId` -> `chat.EditMessage`
- `DELETE /api/messages/:messageId` -> `chat.DeleteMessage`
- `POST /api/conversations/:conversationId/read` -> `chat.MarkConversationRead`

---

# sv

Everything you need to build a Svelte project, powered by [`sv`](https://github.com/sveltejs/cli).

## Creating a project

If you're seeing this, you've probably already done this step. Congrats!

```sh
# create a new project
npx sv create my-app
```

To recreate this project with the same configuration:

```sh
# recreate this project
bun x sv@0.15.1 create --template minimal --types ts --add prettier eslint tailwindcss="plugins:typography" mcp="ide:opencode" vitest="usages:unit,component" --install bun .
```

## Developing

Once you've created a project and installed dependencies with `npm install` (or `pnpm install` or `yarn`), start a development server:

```sh
npm run dev

# or start the server and open the app in a new browser tab
npm run dev -- --open
```

## Building

To create a production version of your app:

```sh
npm run build
```

You can preview the production build with `npm run preview`.

> To deploy your app, you may need to install an [adapter](https://svelte.dev/docs/kit/adapters) for your target environment.
