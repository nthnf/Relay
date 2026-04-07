## Workspace Data Communication Diagram

```mermaid
flowchart LR
    Browser[Browser Client] --> App[External SvelteKit]
    App --> Gateway[Envoy Gateway]
    Gateway -->|gRPC CreateWorkspace / GetWorkspace / ListWorkspacesForUser| Workspace[workspace]
    Gateway -->|gRPC CreateChannel / ListChannels / AddMember / RemoveMember| Workspace
    Gateway -->|gRPC IssueInvitation / AcceptInvitation| Workspace

    subgraph Workspace DB Transaction
        W[(workspace)]
        WM[(workspace_member)]
        WR[(workspace_role)]
        WMR[(workspace_member_role)]
        WI[(workspace_invitation)]
        WC[(workspace_channel)]
        O[(outbox_event)]
    end

    Workspace -->|write workspace metadata| W
    Workspace -->|write memberships| WM
    Workspace -->|write roles and assignments when needed| WR
    Workspace -->|write role assignments| WMR
    Workspace -->|write invitation state| WI
    Workspace -->|write channel metadata| WC
    Workspace -->|same transaction inserts event row| O

    O --> Worker[outbox worker sidecar]
    Worker -->|publish durable workspace events| RabbitMQ[RabbitMQ]
    RabbitMQ -->|WorkspaceCreated / WorkspaceMemberAdded / WorkspaceMemberRemoved / WorkspaceChannelCreated| Bootstrap[bootstrap]
    RabbitMQ -->|selected workspace membership and channel events| Realtime[realtime]
    Bootstrap -->|upsert member-visible workspace and channel projections| Projections[(bootstrap workspace_projection\nworkspace_channel_projection)]
```

Notes:

- Envoy Gateway owns backend ingress policy; workspace owns membership, invitation, role, channel-metadata invariants, and service-boundary authorization.
- Workspace writes domain rows and `outbox_event` rows in the same local Postgres transaction.
- RabbitMQ publication is asynchronous and is the durable path that lets `bootstrap` and `realtime` converge after workspace writes.
- `chat` is intentionally absent from this diagram because workspace owns channel metadata and membership, not message persistence.
- Invitation acceptance can emit both `WorkspaceInvitationAccepted` and `WorkspaceMemberAdded` from one transaction so audit and projection consumers can converge independently.
