# Envoy Gateway With External SvelteKit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Update the documentation set to replace Traefik plus the custom backend gateway service with Envoy Gateway as the backend ingress and an external SvelteKit deployment as the application edge.

**Architecture:** The change preserves backend service ownership, gRPC request/response contracts, RabbitMQ cold-path propagation, and service-local Postgres ownership. The documentation update removes the custom backend gateway from the north-south path, introduces Envoy Gateway as the backend ingress and policy layer, and rewrites service docs so they describe external callers and ingress policy correctly.

**Tech Stack:** Markdown, OpenAPI YAML deletion, Mermaid diagrams, Kubernetes ingress concepts, Envoy Gateway edge model, external SvelteKit server-side gRPC client model

---

## File Structure

### Shared docs to modify

- Modify: `AGENTS.md`
  - Replace the old Traefik plus gateway edge memory with the new Envoy Gateway plus external SvelteKit model.
- Modify: `docs/architecture.md`
  - Rewrite the edge path and ingress responsibilities.
- Modify: `docs/roadmap.md`
  - Replace Traefik/gateway milestones with Envoy Gateway and external-client-facing backend exposure planning.

### Gateway docs to remove

- Delete: `docs/service/gateway/README.md`
- Delete: `docs/service/gateway/data-model.md`
- Delete: `docs/service/gateway/openapi.yml`
- Delete: `docs/service/gateway/events.md`
- Delete: `docs/service/gateway/diagram.md`
- Delete: `docs/service/gateway/user-story.md`
- Delete: `docs/service/gateway/roadmap.md`

### Service docs to modify

- Modify: `docs/service/identity/README.md`
- Modify: `docs/service/identity/data-model.md`
- Modify: `docs/service/identity/grpc.md`
- Modify: `docs/service/identity/diagram.md`
- Modify: `docs/service/identity/roadmap.md`
- Modify: `docs/service/friendship/README.md`
- Modify: `docs/service/friendship/data-model.md`
- Modify: `docs/service/friendship/grpc.md`
- Modify: `docs/service/friendship/diagram.md`
- Modify: `docs/service/workspace/README.md`
- Modify: `docs/service/workspace/data-model.md`
- Modify: `docs/service/workspace/grpc.md`
- Modify: `docs/service/workspace/diagram.md`
- Modify: `docs/service/chat/README.md`
- Modify: `docs/service/chat/data-model.md`
- Modify: `docs/service/chat/grpc.md`
- Modify: `docs/service/chat/diagram.md`
- Modify: `docs/service/realtime/README.md`
- Modify: `docs/service/realtime/data-model.md`
- Modify: `docs/service/realtime/grpc.md`
- Modify: `docs/service/bootstrap/openapi.yml`

### Task 1: Rewrite Shared Edge Architecture Docs

**Files:**
- Modify: `AGENTS.md`
- Modify: `docs/architecture.md`
- Modify: `docs/roadmap.md`

- [ ] **Step 1: Update `AGENTS.md` core memory for the new edge model**

Replace the old edge-memory rule:

```md
- Edge traffic enters through Traefik and then the `gateway` service.
```

with guidance equivalent to:

```md
- Browser traffic reaches an external SvelteKit application.
- Backend traffic enters the Kubernetes cluster through Envoy Gateway.
- There is no custom backend `gateway` service by default in this topology.
```

Keep the rest of the core architecture bullets intact unless they directly depend on the removed gateway.

- [ ] **Step 2: Rewrite `docs/architecture.md` edge sections**

Update these sections in `docs/architecture.md`:

```md
## System Shape
## Edge Path
## Hot Path
## Local Kubernetes Topology
```

Required changes:
- replace Traefik with Envoy Gateway
- remove the statement that `gateway` is the only default north-south entrypoint
- state that SvelteKit is external to the cluster and can call backend gRPC services from its server runtime
- state that Envoy Gateway is the backend ingress and policy boundary
- keep Kubernetes Services as the internal service discovery and load-balancing layer
- keep service-owned authorization distinct from ingress auth

- [ ] **Step 3: Rewrite `docs/roadmap.md` Phase 1 and Phase 2 edge milestones**

Replace gateway/Traefik-specific milestones with wording equivalent to:

```md
- Stand up Envoy Gateway as the backend ingress for the cluster.
- Define which backend gRPC surfaces are externally reachable by SvelteKit and which remain internal-only.
- Define ingress auth, rate-limiting, and route policy boundaries in Envoy Gateway.
- Preserve service-owned authorization and domain invariants inside backend services.
```

Keep the later phases for identity, friendship, workspace, chat, realtime, bootstrap, and email, but remove any dependency on a custom backend gateway service.

- [ ] **Step 4: Verify shared-doc edge wording**

Run: `rg -n "Traefik|gateway service|Envoy Gateway|SvelteKit|north-south" AGENTS.md docs/architecture.md docs/roadmap.md`

Expected:
- shared docs refer to Envoy Gateway and external SvelteKit
- shared docs no longer describe Traefik as the active ingress choice
- shared docs no longer describe the custom backend gateway as the default north-south service

### Task 2: Remove Custom Gateway Service Docs

**Files:**
- Delete: `docs/service/gateway/README.md`
- Delete: `docs/service/gateway/data-model.md`
- Delete: `docs/service/gateway/openapi.yml`
- Delete: `docs/service/gateway/events.md`
- Delete: `docs/service/gateway/diagram.md`
- Delete: `docs/service/gateway/user-story.md`
- Delete: `docs/service/gateway/roadmap.md`

- [ ] **Step 1: Delete the gateway contract folder contents**

Delete the seven files listed above.

Reason:
- the spec removes the custom backend gateway service as the default architecture component
- keeping the old gateway service contracts would directly contradict the new topology

- [ ] **Step 2: Verify that gateway docs are removed**

Run: `rg --files docs/service/gateway`

Expected:
- no files returned

### Task 3: Update Service Docs To Remove Gateway Assumptions

**Files:**
- Modify: `docs/service/identity/README.md`
- Modify: `docs/service/identity/data-model.md`
- Modify: `docs/service/identity/grpc.md`
- Modify: `docs/service/identity/diagram.md`
- Modify: `docs/service/identity/roadmap.md`
- Modify: `docs/service/friendship/README.md`
- Modify: `docs/service/friendship/data-model.md`
- Modify: `docs/service/friendship/grpc.md`
- Modify: `docs/service/friendship/diagram.md`
- Modify: `docs/service/workspace/README.md`
- Modify: `docs/service/workspace/data-model.md`
- Modify: `docs/service/workspace/grpc.md`
- Modify: `docs/service/workspace/diagram.md`
- Modify: `docs/service/chat/README.md`
- Modify: `docs/service/chat/data-model.md`
- Modify: `docs/service/chat/grpc.md`
- Modify: `docs/service/chat/diagram.md`
- Modify: `docs/service/realtime/README.md`
- Modify: `docs/service/realtime/data-model.md`
- Modify: `docs/service/realtime/grpc.md`
- Modify: `docs/service/bootstrap/openapi.yml`

- [ ] **Step 1: Rewrite README edge references in identity, friendship, workspace, chat, and realtime**

Replace phrases like:

```md
behind `gateway`
Acting as the public HTTP edge; `gateway` owns client-facing routing and auth context.
```

with wording equivalent to:

```md
called by external application servers through Envoy Gateway
Envoy Gateway handles backend ingress policy; service-owned authorization remains here
```

Keep the service ownership statements intact.

- [ ] **Step 2: Rewrite gRPC caller descriptions**

In these files:
- `docs/service/identity/grpc.md`
- `docs/service/friendship/grpc.md`
- `docs/service/workspace/grpc.md`
- `docs/service/chat/grpc.md`
- `docs/service/realtime/grpc.md`

Replace main-caller assumptions like:

```md
**Main caller:** `gateway`
Authenticated actor identity comes from `gateway`
```

with concrete caller language such as:

```md
**Main caller:** external application server through Envoy Gateway
Authenticated actor identity is derived from ingress-authenticated request context and must still be authorized by the service boundary
```

Keep service-owned authorization rules explicit.

- [ ] **Step 3: Rewrite data-model and diagram notes that depend on gateway**

Update these files:
- `docs/service/identity/data-model.md`
- `docs/service/friendship/data-model.md`
- `docs/service/workspace/data-model.md`
- `docs/service/chat/data-model.md`
- `docs/service/realtime/data-model.md`
- `docs/service/identity/diagram.md`
- `docs/service/friendship/diagram.md`
- `docs/service/workspace/diagram.md`
- `docs/service/chat/diagram.md`

Required changes:
- remove notes that say gateway forwards authenticated commands
- replace them with ingress-neutral wording that keeps auth context external and service authorization internal
- update diagrams from `Traefik -> gateway -> service` or `gateway -> service` to either:
  - `External App -> Envoy Gateway -> Service`
  - or `External App -> Service via Envoy Gateway`

- [ ] **Step 4: Update bootstrap public-surface wording**

In `docs/service/bootstrap/openapi.yml`, replace the server description:

```yaml
description: Public endpoint through Traefik and gateway
```

with wording equivalent to:

```yaml
description: Backend ingress endpoint exposed through Envoy Gateway for external application callers
```

- [ ] **Step 5: Verify service-doc edge references are normalized**

Run: `rg -n "Traefik|gateway|Envoy Gateway|external application server|ingress-authenticated request context" docs/service`

Expected:
- no surviving references that treat the custom gateway service as the live architecture
- gateway references only remain where historical/spec files outside service docs are intentionally untouched
- updated service docs describe external callers and Envoy Gateway appropriately

### Task 4: Final Cross-Doc Verification

**Files:**
- Verify: `AGENTS.md`
- Verify: `docs/architecture.md`
- Verify: `docs/roadmap.md`
- Verify: `docs/service/**`

- [ ] **Step 1: Verify no active production docs still describe Traefik plus custom gateway as the live edge path**

Run: `rg -n "Traefik|gateway service|gateway owns client-facing|through Traefik and gateway" AGENTS.md docs/architecture.md docs/roadmap.md docs/service`

Expected:
- no active platform/service docs still describe the removed edge model

- [ ] **Step 2: Verify the new edge model is present**

Run: `rg -n "Envoy Gateway|SvelteKit|external application|backend ingress|service-owned authorization" AGENTS.md docs/architecture.md docs/roadmap.md docs/service`

Expected:
- shared docs and affected service docs clearly describe the new external-SvelteKit plus Envoy-Gateway topology

- [ ] **Step 3: Verify docs directory shape after gateway removal**

Run: `rg --files docs/service | sort`

Expected:
- service docs remain for `bootstrap`, `identity`, `friendship`, `workspace`, `chat`, `realtime`, and `email`
- `docs/service/gateway/` no longer contributes active contract files

- [ ] **Step 4: Summarize residual risks for the user**

Call out these exact residual design risks if they still apply after edits:
- external SvelteKit may become tightly coupled to too many backend services if ingress exposure is not kept narrow
- ingress authentication must not be mistaken for full domain authorization
- backend public exposure must remain explicit rather than accidental
