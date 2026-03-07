#[cfg(unix)]
use anyhow::Context;
#[cfg(unix)]
use backup_core::config::model::Destination;
#[cfg(unix)]
use daemon::runtime::socket_path;
#[cfg(unix)]
use daemon::runtime::{cid, spawn_server};
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use std::sync::OnceLock;
#[cfg(unix)]
use std::time::{Duration, Instant};
#[cfg(unix)]
use tempfile::{tempdir, TempDir};
#[cfg(unix)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(unix)]
use tokio::sync::{Mutex, MutexGuard};
#[cfg(unix)]
use tracing::warn;

#[cfg(unix)]
/// Summary: Bundles daemon IPC test server lifecycle handles.
///
/// Inputs: none.
///
/// Outputs: socket path and shutdown handles for tests.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `spawn_test_server` and `shutdown`.
///
/// Why this exists: keep IPC test lifecycle state cohesive and easy to pass around.
pub struct IpcTestServer {
    _temp_dir: TempDir,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    handle: tokio::task::JoinHandle<()>,
    path: PathBuf,
}

#[cfg(unix)]
impl IpcTestServer {
    /// Summary: Returns the Unix socket path used by this test server.
    ///
    /// Inputs: none.
    ///
    /// Outputs: reference to the socket path.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `send_json_request`.
    ///
    /// Why this exists: avoid exposing internal lifecycle fields directly.
    pub fn socket_path(&self) -> &Path {
        &self.path
    }

    /// Summary: Stops the IPC test server and verifies socket cleanup.
    ///
    /// Inputs: timeout budget for the async join.
    ///
    /// Outputs: none.
    ///
    /// Side effects: Signals shutdown and waits for daemon server task completion.
    ///
    /// Error handling: Panics with contextual messages when shutdown fails or times out.
    ///
    /// Ties to other methods: `spawn_test_server`.
    ///
    /// Why this exists: centralize deterministic teardown logic for IPC integration tests.
    pub async fn shutdown(self, timeout: Duration) {
        self.shutdown_tx
            .send(true)
            .expect("daemon::tests::support::IpcTestServer::shutdown failed to signal shutdown");
        let join = tokio::time::timeout(timeout, self.handle).await;
        assert!(
            join.is_ok(),
            "daemon::tests::support::IpcTestServer::shutdown timed out"
        );
        assert!(
            !self.path.exists(),
            "daemon::tests::support::IpcTestServer::shutdown expected socket path cleanup"
        );
    }
}

#[cfg(unix)]
/// Summary: Starts a daemon IPC server for integration tests.
///
/// Inputs: test name for contextual warning logging.
///
/// Outputs: server lifecycle bundle, or `None` when bind conflicts occur.
///
/// Side effects: Creates temporary directories, initializes state, and binds local IPC socket.
///
/// Error handling: Returns `None` on bind errors so tests can skip host-specific conflicts.
///
/// Ties to other methods: `IpcTestServer::shutdown`, `send_json_request`.
///
/// Why this exists: remove duplicated setup boilerplate across IPC test files.
pub async fn spawn_test_server(test_name: &str) -> Option<IpcTestServer> {
    let tmp = tempdir().expect("daemon::tests::support::spawn_test_server temp dir");
    let backup_root = tmp.path().join("backups");
    std::fs::create_dir_all(&backup_root)
        .expect("daemon::tests::support::spawn_test_server backup root");
    let (state, _) = backup_core::StateStore::load_or_default(tmp.path().join("ipc_state.json"))
        .expect("daemon::tests::support::spawn_test_server load state");
    let shared = Arc::new(tokio::sync::Mutex::new(state));
    let destinations = vec![Destination {
        id: "default".into(),
        label: Some("Primary".into()),
        path: backup_root,
        max_backups_per_file: None,
        replicate_to: vec![],
    }];
    let runtime_defaults = backup_core::config::model::RuntimeTuning::default();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let handle = match spawn_server(
        shared,
        destinations,
        Duration::from_secs(5),
        runtime_defaults.ipc_request_max_bytes,
        runtime_defaults.ipc_read_chunk_bytes,
        10,
        shutdown_rx,
    )
    .await
    {
        Ok(handle) => handle,
        Err(error) => {
            let correlation_id = cid("daemon-test-ipc");
            warn!(
                cid = %correlation_id,
                action = "skip_bind_error",
                test = test_name,
                error = %error,
                "skipping IPC test due to bind error"
            );
            return None;
        }
    };
    let path = socket_path().expect("daemon::tests::support::spawn_test_server socket path");
    Some(IpcTestServer {
        _temp_dir: tmp,
        shutdown_tx,
        handle,
        path,
    })
}

#[cfg(unix)]
/// Summary: Returns a process-local IPC lock so tests using the shared socket path run serially.
///
/// Inputs: none.
///
/// Outputs: mutex guard held for the caller lifetime.
///
/// Side effects: serializes IPC integration tests inside the current test process.
///
/// Error handling: Panics with contextual method/file messaging when lock acquisition fails.
///
/// Ties to other methods: daemon IPC integration tests using `daemon::runtime::socket_path`.
///
/// Why this exists: avoid socket path collisions from parallel tests.
pub async fn ipc_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().await
}

#[cfg(unix)]
/// Summary: Removes a stale Unix socket file before or after a test run.
///
/// Inputs: none.
///
/// Outputs: none.
///
/// Side effects: Deletes the daemon IPC socket file if it exists.
///
/// Error handling: Panics with contextual method/file messaging when deletion fails.
///
/// Ties to other methods: test setup/teardown around daemon IPC server lifecycle.
///
/// Why this exists: keep IPC integration tests isolated across repeated local runs.
pub fn cleanup_socket_file() {
    let path = socket_path().expect("daemon::tests::support::cleanup_socket_file socket path");
    if path.exists() {
        std::fs::remove_file(&path).unwrap_or_else(|error| {
            panic!(
                "daemon::tests::support::cleanup_socket_file remove stale socket failed: {error}"
            )
        });
    }
}

#[cfg(unix)]
/// Summary: Connects to a Unix socket with bounded retry so tests wait for listener readiness.
///
/// Inputs: socket path and total retry budget.
///
/// Outputs: connected `UnixStream`.
///
/// Side effects: Performs repeated local connection attempts with short sleeps.
///
/// Error handling: Returns contextual errors when connect attempts exceed timeout.
///
/// Ties to other methods: IPC contract tests that must avoid fixed startup sleeps.
///
/// Why this exists: improve determinism versus race-prone fixed delays.
pub async fn connect_with_retry(path: &Path, timeout: Duration) -> anyhow::Result<UnixStream> {
    let start = Instant::now();
    loop {
        match UnixStream::connect(path).await {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                if start.elapsed() >= timeout {
                    return Err(anyhow::anyhow!(error).context(format!(
                        "daemon::tests::support::connect_with_retry timed out connecting to {:?}",
                        path
                    )));
                }
                tokio::time::sleep(Duration::from_millis(15)).await;
            }
        }
    }
}

#[cfg(unix)]
/// Summary: Sends a JSON IPC request and returns the parsed JSON reply.
///
/// Inputs: socket path, request payload, and timeout budget.
///
/// Outputs: parsed reply payload as `serde_json::Value`.
///
/// Side effects: Opens a Unix socket client connection and performs one request/response exchange.
///
/// Error handling: Returns contextual errors for connection, IO, timeout, or parse failures.
///
/// Ties to other methods: daemon IPC contract assertions for status/health/readiness/ack replies.
///
/// Why this exists: centralize deterministic request/response flow for IPC integration tests.
pub async fn send_json_request(
    path: &Path,
    request: &serde_json::Value,
    timeout: Duration,
) -> anyhow::Result<serde_json::Value> {
    tokio::time::timeout(timeout, async {
        let request_text = serde_json::to_string(request)
            .context("daemon::tests::support::send_json_request serialize request")?;
        let mut stream = connect_with_retry(path, timeout).await?;
        stream
            .write_all(request_text.as_bytes())
            .await
            .context("daemon::tests::support::send_json_request write request")?;
        stream
            .shutdown()
            .await
            .context("daemon::tests::support::send_json_request shutdown write stream")?;
        let mut buf = Vec::new();
        stream
            .read_to_end(&mut buf)
            .await
            .context("daemon::tests::support::send_json_request read response")?;
        serde_json::from_slice(&buf)
            .context("daemon::tests::support::send_json_request parse response JSON")
    })
    .await
    .context("daemon::tests::support::send_json_request timed out")?
}
