#[allow(dead_code)]
mod common;

use std::net::{IpAddr, Ipv4Addr};

use chrono::{Duration, Utc};
use common::{
    TestEnv, access_token, hash_token_for_test, insert_user_account, insert_user_profile,
    insert_user_session,
};
use testcontainers::{
    GenericImage, ImageExt,
    core::{CopyDataSource, IntoContainerPort},
    runners::AsyncRunner,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn envoy_config(auth_port: u16, upstream_port: u16) -> String {
    format!(
        r#"
static_resources:
  listeners:
  - name: listener_0
    address:
      socket_address:
        address: 0.0.0.0
        port_value: 10000
    filter_chains:
    - filters:
      - name: envoy.filters.network.http_connection_manager
        typed_config:
          "@type": type.googleapis.com/envoy.extensions.filters.network.http_connection_manager.v3.HttpConnectionManager
          stat_prefix: ingress_http
          codec_type: AUTO
          route_config:
            name: local_route
            virtual_hosts:
            - name: local_service
              domains: ["*"]
              routes:
              - match:
                  prefix: "/"
                route:
                  cluster: upstream-service
          http_filters:
          - name: envoy.filters.http.ext_authz
            typed_config:
              "@type": type.googleapis.com/envoy.extensions.filters.http.ext_authz.v3.ExtAuthz
              stat_prefix: ext_authz
              transport_api_version: V3
              grpc_service:
                envoy_grpc:
                  cluster_name: ext-authz
                timeout: 2s
          - name: envoy.filters.http.router
            typed_config:
              "@type": type.googleapis.com/envoy.extensions.filters.http.router.v3.Router
  clusters:
  - name: upstream-service
    type: logical_dns
    dns_lookup_family: V4_ONLY
    lb_policy: ROUND_ROBIN
    load_assignment:
      cluster_name: upstream-service
      endpoints:
      - lb_endpoints:
        - endpoint:
            address:
              socket_address:
                address: host.testcontainers.internal
                port_value: {upstream_port}
  - name: ext-authz
    type: logical_dns
    dns_lookup_family: V4_ONLY
    lb_policy: ROUND_ROBIN
    typed_extension_protocol_options:
      envoy.extensions.upstreams.http.v3.HttpProtocolOptions:
        "@type": type.googleapis.com/envoy.extensions.upstreams.http.v3.HttpProtocolOptions
        explicit_http_config:
          http2_protocol_options: {{}}
    load_assignment:
      cluster_name: ext-authz
      endpoints:
      - lb_endpoints:
        - endpoint:
            address:
              socket_address:
                address: host.testcontainers.internal
                port_value: {auth_port}
"#
    )
}

async fn http_request(
    port: u16,
    token: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await?;
    let mut request = String::from("GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n");
    if let Some(token) = token {
        request.push_str(&format!("authorization: Bearer {token}\r\n"));
    }
    request.push_str("\r\n");

    stream.write_all(request.as_bytes()).await?;
    let mut response = String::new();
    stream.read_to_string(&mut response).await?;
    Ok(response)
}

async fn envoy_logs(
    envoy: &testcontainers::ContainerAsync<GenericImage>,
) -> Result<String, Box<dyn std::error::Error>> {
    let stdout_bytes = envoy.stdout_to_vec().await?;
    let stderr_bytes = envoy.stderr_to_vec().await?;
    let stdout = String::from_utf8_lossy(&stdout_bytes);
    let stderr = String::from_utf8_lossy(&stderr_bytes);

    Ok(format!("stdout:\n{stdout}\nstderr:\n{stderr}"))
}

async fn eventually_http_request(
    port: u16,
    token: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut last_error = None;

    for _ in 0..30 {
        match http_request(port, token).await {
            Ok(response) => return Ok(response),
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }

    Err(last_error.expect("request should have been attempted"))
}

async fn wait_for_tcp_port(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let mut last_error = None;

    for _ in 0..30 {
        match TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await {
            Ok(_) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }

    Err(Box::new(
        last_error.expect("connection should have been attempted"),
    ))
}

async fn start_upstream_echo_server()
-> Result<(u16, tokio::task::JoinHandle<()>), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();

    let task = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buffer = vec![0; 8192];
                let Ok(bytes_read) = stream.read(&mut buffer).await else {
                    return;
                };
                let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                let user_id = request
                    .lines()
                    .find_map(|line| line.strip_prefix("x-user-id: "))
                    .unwrap_or("missing");
                let body = format!("x-user-id:{user_id}");
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );

                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });

    Ok((port, task))
}

async fn seed_active_session(
    env: &TestEnv,
) -> Result<(uuid::Uuid, String), Box<dyn std::error::Error>> {
    let now = Utc::now();
    let user_id = uuid::Uuid::new_v4();
    let session_id = uuid::Uuid::new_v4();

    insert_user_account(
        &env.db,
        user_id,
        "envoy@example.com",
        Some(now),
        "active",
        now,
    )
    .await?;
    insert_user_profile(&env.db, user_id, "envoy-user", "Envoy User", None, now).await?;
    insert_user_session(
        &env.db,
        session_id,
        user_id,
        hash_token_for_test("refresh-token"),
        now,
        now + Duration::days(7),
        None,
        None,
        None,
        None,
    )
    .await?;

    Ok((user_id, access_token(user_id, session_id)))
}

#[tokio::test]
async fn envoy_ext_authz_allows_and_denies_requests() -> Result<(), Box<dyn std::error::Error>> {
    let env = TestEnv::start_with_bind_ip(IpAddr::V4(Ipv4Addr::UNSPECIFIED)).await?;
    let (user_id, token) = seed_active_session(&env).await?;
    let auth_port = env.listen_addr.port();
    let (upstream_port, upstream_task) = start_upstream_echo_server().await?;
    wait_for_tcp_port(auth_port).await?;
    wait_for_tcp_port(upstream_port).await?;

    let envoy_yaml = envoy_config(auth_port, upstream_port);
    let envoy = GenericImage::new("envoyproxy/envoy", "v1.31-latest")
        .with_exposed_port(10000.tcp())
        .with_exposed_host_ports([auth_port, upstream_port])
        .with_copy_to(
            "/etc/envoy/envoy.yaml",
            CopyDataSource::Data(envoy_yaml.into_bytes()),
        )
        .start()
        .await?;
    let envoy_port = envoy.get_host_port_ipv4(10000.tcp()).await?;

    let allowed = match eventually_http_request(envoy_port, Some(&token)).await {
        Ok(response) => response,
        Err(error) => {
            let logs = envoy_logs(&envoy).await?;
            panic!("Envoy did not accept requests: {error}\n{logs}");
        }
    };
    if !allowed.starts_with("HTTP/1.1 200") {
        let logs = envoy_logs(&envoy).await?;
        panic!("unexpected response: {allowed}\n{logs}");
    }
    assert!(
        allowed.contains(&format!("x-user-id:{user_id}")),
        "upstream did not receive x-user-id header: {allowed}"
    );

    let denied = eventually_http_request(envoy_port, None).await?;
    assert!(
        denied.starts_with("HTTP/1.1 403"),
        "unexpected response: {denied}"
    );

    upstream_task.abort();
    env.shutdown().await;
    Ok(())
}
