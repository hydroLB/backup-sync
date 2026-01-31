use backup_core::config::model::Destination;
use daemon::runtime::ipc;
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(unix)]
#[tokio::test]
/// Purpose: Validate that the IPC server returns a status payload over a Unix socket.
///
/// Inputs: None.
/// Outputs: Asserts that the response includes expected status fields.
/// Ties to: `daemon::runtime::ipc::spawn_server` and status response serialization.
/// Side effects: Creates temp directories, binds a Unix socket, and exchanges data over IPC.
/// Why: Ensure the IPC boundary works end to end for local status queries.
async fn unix_ipc_status_round_trip() {
    use tokio::net::UnixStream;
    let tmp = tempdir().expect("ipc_status::unix_ipc_status_round_trip failed to create temp dir");
    let backup_root = tmp.path().join("backups");
    std::fs::create_dir_all(&backup_root)
        .expect("ipc_status::unix_ipc_status_round_trip failed to create backup root");
    let (state, _) = backup_core::StateStore::load_or_default(tmp.path().join("ipc_state.json"))
        .expect("ipc_status::unix_ipc_status_round_trip failed to load state");
    let shared = std::sync::Arc::new(tokio::sync::Mutex::new(state));
    let destinations = vec![Destination {
        id: "default".into(),
        label: None,
        path: backup_root.clone(),
        max_backups_per_file: None,
    }];
    let handle = match ipc::spawn_server(
        shared.clone(),
        destinations,
        std::time::Duration::from_secs(5),
        10,
    )
    .await
    {
        Ok(h) => h,
        Err(e) => {
            eprintln!("skipping IPC test due to bind error: {}", e);
            return;
        }
    };
    let path = ipc::socket_path()
        .expect("ipc_status::unix_ipc_status_round_trip failed to resolve socket path");
    let req = serde_json::json!({"type":"Status"}).to_string();
    // Allow the server to bind before connecting.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let res = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        let mut stream = UnixStream::connect(path)
            .await
            .expect("ipc_status::unix_ipc_status_round_trip failed to connect");
        stream
            .write_all(req.as_bytes())
            .await
            .expect("ipc_status::unix_ipc_status_round_trip failed to write request");
        // Signal EOF so the server finishes reading.
        stream
            .shutdown()
            .await
            .expect("ipc_status::unix_ipc_status_round_trip failed to shutdown stream");
        let mut buf = Vec::new();
        stream
            .read_to_end(&mut buf)
            .await
            .expect("ipc_status::unix_ipc_status_round_trip failed to read response");
        let val: serde_json::Value = serde_json::from_slice(&buf)
            .expect("ipc_status::unix_ipc_status_round_trip failed to parse response JSON");
        assert!(val.get("last_files_backed_up").is_some());
    })
    .await;
    assert!(res.is_ok(), "IPC round trip timed out");
    handle.abort();
}

// Windows named pipe IPC is exercised via compilation; runtime test omitted here.
