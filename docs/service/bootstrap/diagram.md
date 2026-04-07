```mermaid
flowchart LR
    Client[Browser Client]
    App[External SvelteKit]
    Gateway[Envoy Gateway]
    Bootstrap[bootstrap]
    RabbitMQ[RabbitMQ]
    Projections[(bootstrap projections\nuser_home_projection\nfriend_projection\nworkspace_projection\nworkspace_channel_projection\nuser_unread_counter)]

    Client -->|HTTP app request| App
    App -->|approved backend gRPC read| Gateway
    Gateway -->|GetHome / ListFriends / ListWorkspaces / GetWorkspaceSidebar| Bootstrap
    Bootstrap -->|read denormalized rows| Projections

    RabbitMQ -->|UserRegistered\nUserProfileUpdated\nFriendRequestAccepted\nFriendshipRemoved\nWorkspaceCreated\nWorkspaceMemberAdded\nWorkspaceMemberRemoved\nWorkspaceChannelCreated\nMessageCreated\nMessageEdited\nMessageDeleted| Bootstrap
    Bootstrap -->|upsert projection rows| Projections
    Bootstrap -->|compute unread/preview from event payload + local membership projections| Projections
```
