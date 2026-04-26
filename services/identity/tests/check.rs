#[allow(dead_code)]
mod common;

use chrono::{Duration, Utc};
use common::{
    TestEnv, access_token, insert_user_account, insert_user_profile, insert_user_session,
};
use envoy_types::ext_authz::v3::pb::{CheckRequest, CheckResponse, HttpResponse};
use envoy_types::pb::envoy::service::auth::v3::{
    AttributeContext,
    attribute_context::{HttpRequest, Request as AuthRequest},
};
use std::collections::HashMap;
use uuid::Uuid;

fn check_request(token: &str) -> tonic::Request<CheckRequest> {
    tonic::Request::new(CheckRequest {
        attributes: Some(AttributeContext {
            request: Some(AuthRequest {
                http: Some(HttpRequest {
                    headers: std::iter::once((
                        "authorization".to_string(),
                        format!("Bearer {token}"),
                    ))
                    .collect(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
    })
}

fn assert_denied_check(response: &CheckResponse, message: &str) {
    let status = response.status.as_ref().expect("status");
    assert_eq!(status.code, tonic::Code::Unauthenticated as i32);
    assert_eq!(status.message, message);
    assert!(response.http_response.is_none());
}

#[tokio::test]
async fn check_accepts_active_session() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();

    insert_user_account(
        &env.db,
        user_id,
        "alice@example.com",
        Some(now),
        "active",
        now,
    )
    .await?;
    insert_user_profile(&env.db, user_id, "alice", "Alice", None, now).await?;
    insert_user_session(
        &env.db,
        session_id,
        user_id,
        common::hash_token_for_test("refresh-token"),
        now,
        now + Duration::days(7),
        None,
        None,
        None,
        None,
    )
    .await?;

    let response = env
        .auth_client
        .clone()
        .check(check_request(&access_token(user_id, session_id)))
        .await?
        .into_inner();

    let status = response.status.clone().expect("status");
    let ok_response = match response.http_response {
        Some(HttpResponse::OkResponse(ok_response)) => ok_response,
        other => panic!("expected ok http response, got {other:?}"),
    };
    let headers = ok_response
        .headers
        .iter()
        .filter_map(|header| {
            header
                .header
                .as_ref()
                .map(|value| (value.key.as_str(), value.value.as_str()))
        })
        .collect::<HashMap<_, _>>();

    assert_eq!(status.code, tonic::Code::Ok as i32);
    assert_eq!(status.message, "request is valid");
    assert_eq!(
        headers.get("x-user-id").copied(),
        Some(user_id.to_string().as_str())
    );
    assert_eq!(
        headers.get("x-session-id").copied(),
        Some(session_id.to_string().as_str())
    );

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn check_rejects_unknown_user() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let token = access_token(Uuid::new_v4(), Uuid::new_v4());

    let response = env
        .auth_client
        .clone()
        .check(check_request(&token))
        .await?
        .into_inner();

    assert_denied_check(&response, "unknown user");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn check_rejects_inactive_account() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();

    insert_user_account(
        &env.db,
        user_id,
        "alice@example.com",
        Some(now),
        "disabled",
        now,
    )
    .await?;
    insert_user_session(
        &env.db,
        session_id,
        user_id,
        common::hash_token_for_test("refresh-token"),
        now,
        now + Duration::days(7),
        None,
        None,
        None,
        None,
    )
    .await?;

    let response = env
        .auth_client
        .clone()
        .check(check_request(&access_token(user_id, session_id)))
        .await?
        .into_inner();

    assert_denied_check(&response, "account is not active");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn check_rejects_unknown_session() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();

    insert_user_account(
        &env.db,
        user_id,
        "alice@example.com",
        Some(now),
        "active",
        now,
    )
    .await?;

    let response = env
        .auth_client
        .clone()
        .check(check_request(&access_token(user_id, session_id)))
        .await?
        .into_inner();

    assert_denied_check(&response, "unknown session");

    env.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn check_rejects_revoked_session() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start().await?;
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();

    insert_user_account(
        &env.db,
        user_id,
        "alice@example.com",
        Some(now),
        "active",
        now,
    )
    .await?;
    insert_user_session(
        &env.db,
        session_id,
        user_id,
        common::hash_token_for_test("refresh-token"),
        now,
        now + Duration::days(7),
        Some(now),
        Some("logout"),
        None,
        None,
    )
    .await?;

    let response = env
        .auth_client
        .clone()
        .check(check_request(&access_token(user_id, session_id)))
        .await?
        .into_inner();

    assert_denied_check(&response, "session revoked");

    env.shutdown().await;
    Ok(())
}
