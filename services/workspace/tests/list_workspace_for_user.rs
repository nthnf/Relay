extern crate workspace as workspace_crate;

use relay_proto::workspace::workspace_service_client::WorkspaceServiceClient;
use relay_proto::workspace::{CreateWorkspaceRequest, ListWorkspacesForUserRequest};
use sea_orm::{Database, EntityTrait};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{core::IntoContainerPort, runners::AsyncRunner},
};
use tonic::{metadata::MetadataValue, transport::Server, Request};
use uuid::Uuid;

use migration::{Migrator, MigratorTrait};
use workspace_crate::entity::user_snapshot;
use workspace_crate::grpc::WorkspaceServer;

const ACTOR_USER_ID_METADATA: &str = "x-user-id";

#[tokio::test]
async fn list_workspaces_for_user_returns_paginated_summaries()
-> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let actor_user_id = Uuid::new_v4();

    insert_user_snapshot(&env.db, actor_user_id).await?;

    let first = env
        .client
        .clone()
        .create_workspace(actor_request(
            actor_user_id,
            CreateWorkspaceRequest {
                name: "Alpha".to_string(),
                first_channel_name: "general".to_string(),
            },
        ))
        .await?
        .into_inner();

    let second = env
        .client
        .clone()
        .create_workspace(actor_request(
            actor_user_id,
            CreateWorkspaceRequest {
                name: "Beta".to_string(),
                first_channel_name: "general".to_string(),
            },
        ))
        .await?
        .into_inner();

    let page_one = env
        .client
        .clone()
        .list_workspaces_for_user(actor_request(
            actor_user_id,
            ListWorkspacesForUserRequest {
                page_size: Some(1),
                page_token: None,
            },
        ))
        .await?
        .into_inner();

    assert_eq!(page_one.workspaces.len(), 1);
    assert_eq!(page_one.workspaces[0].member_count, 1);
    assert_eq!(page_one.workspaces[0].channel_count, 1);
    assert!(page_one.workspaces[0].joined_at.is_some());
    assert!(page_one.next_page_token.is_some());

    let page_two = env
        .client
        .clone()
        .list_workspaces_for_user(actor_request(
            actor_user_id,
            ListWorkspacesForUserRequest {
                page_size: Some(1),
                page_token: page_one.next_page_token.clone(),
            },
        ))
        .await?
        .into_inner();

    assert_eq!(page_two.workspaces.len(), 1);

    let names = [page_one.workspaces[0].name.clone(), page_two.workspaces[0].name.clone()];
    assert!(names.contains(&"Alpha".to_string()));
    assert!(names.contains(&"Beta".to_string()));

    // Final full-page sanity: both workspace IDs must be present across both pages.
    let ids = [
        page_one.workspaces[0].workspace_id.clone(),
        page_two.workspaces[0].workspace_id.clone(),
    ];
    assert!(ids.contains(&first.workspace_id));
    assert!(ids.contains(&second.workspace_id));

    env.shutdown().await;
    Ok(())
}

struct TestEnv {
    _postgres: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    db: sea_orm::DatabaseConnection,
    client: WorkspaceServiceClient<tonic::transport::Channel>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    server_task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

impl TestEnv {
    async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let postgres = Postgres::default().start().await?;
        let postgres_host = postgres.get_host().await?;
        let postgres_port = postgres.get_host_port_ipv4(5432.tcp()).await?;
        let database_url =
            format!("postgres://postgres:postgres@{postgres_host}:{postgres_port}/postgres");

        let db = Database::connect(&database_url).await?;
        Migrator::up(&db, None).await?;

        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        drop(listener);

        let handler = WorkspaceServer::new(db.clone());
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let server_task = tokio::spawn(async move {
            Server::builder()
                .add_service(handler.into_server())
                .serve_with_shutdown(addr, async {
                    let _ = shutdown_rx.await;
                })
                .await
        });

        let client = connect_client(addr).await?;

        Ok(Self {
            _postgres: postgres,
            db,
            client,
            shutdown: Some(shutdown_tx),
            server_task,
        })
    }

    async fn shutdown(mut self) {
        if let Some(sender) = self.shutdown.take() {
            let _ = sender.send(());
        }

        let _ = self.server_task.await;
    }
}

async fn connect_client(
    addr: std::net::SocketAddr,
) -> Result<WorkspaceServiceClient<tonic::transport::Channel>, Box<dyn std::error::Error>> {
    let endpoint = format!("http://{addr}");

    for _ in 0..20 {
        match WorkspaceServiceClient::connect(endpoint.clone()).await {
            Ok(client) => return Ok(client),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }

    Ok(WorkspaceServiceClient::connect(endpoint).await?)
}

fn actor_request<T>(user_id: Uuid, request: T) -> Request<T> {
    let mut request = Request::new(request);
    request
        .metadata_mut()
        .insert(
            ACTOR_USER_ID_METADATA,
            MetadataValue::try_from(user_id.to_string()).expect("metadata"),
        );
    request
}

async fn insert_user_snapshot(
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    workspace_crate::entity::user_snapshot::Entity::insert(user_snapshot::ActiveModel {
        user_id: sea_orm::Set(user_id),
        email_verified: sea_orm::Set(false),
        username: sea_orm::Set(format!("user-{user_id}")),
        display_name: sea_orm::Set("Test User".to_string()),
        avatar_url: sea_orm::Set(None),
        created_at: sea_orm::Set(now.into()),
        updated_at: sea_orm::Set(now.into()),
    })
    .exec(db)
    .await?;

    Ok(())
}
