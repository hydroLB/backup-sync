#[cfg(unix)]
mod support;

#[cfg(unix)]
const IPC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[cfg(unix)]
#[tokio::test]
/// Protect machine-consumed IPC contract fields from accidental regressions.
async fn unix_ipc_public_contract_status_health_readiness() {
    let _ipc_lock = support::ipc_test_lock().await;
    let server =
        support::spawn_test_server("unix_ipc_public_contract_status_health_readiness").await;
    let path = server.socket_path();

    let status = support::send_json_request(
        path,
        &serde_json::json!({
            "type":"StatusWithContext",
            "payload":{"request_id":"status-contract-1","source":"test-suite"}
        }),
        IPC_TIMEOUT,
    )
    .await
    .expect("ipc_status::unix_ipc_public_contract_status_health_readiness status request failed");
    assert_eq!(status["request_id"], "status-contract-1");
    assert!(status["last_files_backed_up"].as_u64().is_some());
    assert!(status["destination_paused"].as_bool().is_some());
    let destinations = status["destinations"].as_array().expect(
        "ipc_status::unix_ipc_public_contract_status_health_readiness expected destinations array",
    );
    assert_eq!(destinations.len(), 1);
    assert!(destinations[0]["id"].as_str().is_some());
    assert!(destinations[0]["path"].as_str().is_some());
    assert!(destinations[0]["reachable"].as_bool().is_some());
    assert!(destinations[0]["writable"].as_bool().is_some());
    assert!(destinations[0]["message"].as_str().is_some());

    let health = support::send_json_request(
        path,
        &serde_json::json!({
            "type":"HealthWithContext",
            "payload":{"request_id":"health-contract-1","source":"test-suite"}
        }),
        IPC_TIMEOUT,
    )
    .await
    .expect("ipc_status::unix_ipc_public_contract_status_health_readiness health request failed");
    assert_eq!(health["request_id"], "health-contract-1");
    assert_eq!(health["healthy"], true);
    assert_eq!(health["status"], "healthy");
    assert!(health["checked_at_ts"].as_i64().is_some());

    let readiness = support::send_json_request(
        path,
        &serde_json::json!({
            "type":"ReadinessWithContext",
            "payload":{"request_id":"readiness-contract-1","source":"test-suite"}
        }),
        IPC_TIMEOUT,
    )
    .await
    .expect(
        "ipc_status::unix_ipc_public_contract_status_health_readiness readiness request failed",
    );
    assert_eq!(readiness["request_id"], "readiness-contract-1");
    assert_eq!(readiness["ready"], true);
    assert_eq!(readiness["status"], "ready");

    server.shutdown(IPC_TIMEOUT).await;
}

#[cfg(unix)]
#[tokio::test]
/// Ensure automation clients can safely depend on safe-mode state transitions.
async fn unix_ipc_public_contract_set_safe_mode_ack_and_readiness_transition() {
    let _ipc_lock = support::ipc_test_lock().await;
    let server = support::spawn_test_server(
        "unix_ipc_public_contract_set_safe_mode_ack_and_readiness_transition",
    )
    .await;
    let path = server.socket_path();

    let ack_enable = support::send_json_request(
        path,
        &serde_json::json!({
            "type":"SetSafeMode",
            "payload":{"enabled":true,"request_id":"set-safe-mode-1","source":"test-suite"}
        }),
        IPC_TIMEOUT,
    )
    .await
    .expect("ipc_status::unix_ipc_public_contract_set_safe_mode_ack_and_readiness_transition enable ack request failed");
    assert_eq!(ack_enable["ok"], true);
    assert_eq!(ack_enable["request_id"], "set-safe-mode-1");

    let readiness_blocked = support::send_json_request(
        path,
        &serde_json::json!({
            "type":"ReadinessWithContext",
            "payload":{"request_id":"readiness-after-enable","source":"test-suite"}
        }),
        IPC_TIMEOUT,
    )
    .await
    .expect("ipc_status::unix_ipc_public_contract_set_safe_mode_ack_and_readiness_transition readiness-after-enable request failed");
    assert_eq!(readiness_blocked["ready"], false);
    assert_eq!(readiness_blocked["status"], "not_ready");
    assert_eq!(readiness_blocked["reason"], "safe_mode_enabled");
    assert_eq!(readiness_blocked["request_id"], "readiness-after-enable");

    let ack_disable = support::send_json_request(
        path,
        &serde_json::json!({
            "type":"SetSafeMode",
            "payload":{"enabled":false,"request_id":"set-safe-mode-2","source":"test-suite"}
        }),
        IPC_TIMEOUT,
    )
    .await
    .expect("ipc_status::unix_ipc_public_contract_set_safe_mode_ack_and_readiness_transition disable ack request failed");
    assert_eq!(ack_disable["ok"], true);
    assert_eq!(ack_disable["request_id"], "set-safe-mode-2");

    let readiness_ready = support::send_json_request(
        path,
        &serde_json::json!({
            "type":"ReadinessWithContext",
            "payload":{"request_id":"readiness-after-disable","source":"test-suite"}
        }),
        IPC_TIMEOUT,
    )
    .await
    .expect("ipc_status::unix_ipc_public_contract_set_safe_mode_ack_and_readiness_transition readiness-after-disable request failed");
    assert_eq!(readiness_ready["ready"], true);
    assert_eq!(readiness_ready["status"], "ready");
    assert_eq!(readiness_ready["request_id"], "readiness-after-disable");

    server.shutdown(IPC_TIMEOUT).await;
}

// Windows named pipe IPC is exercised via compilation; runtime test omitted here.
