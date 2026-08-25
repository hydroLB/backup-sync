#[cfg(unix)]
use anyhow::Context;
#[cfg(unix)]
use backup_core::config::model::Destination;
#[cfg(unix)]
use daemon::runtime::socket_path;
#[cfg(unix)]
use daemon::runtime::spawn_server;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
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
/// Keep IPC test lifecycle state cohesive and easy to pass around.
pub struct IpcTestServer {
    _temp_dir: TempDir,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    handle: tokio::task::JoinHandle<()>,
    path: PathBuf,
    previous_socket_path: Option<std::ffi::OsString>,
}

#[cfg(unix)]
impl IpcTestServer {
    /// Avoid exposing internal lifecycle fields directly.
    pub fn socket_path(&self) -> &Path {
        &self.path
    }

    /// Centralize deterministic teardown logic for IPC integration tests.
    pub async fn shutdown(self, timeout: Duration) {
        self.shutdown_tx
            .send(true)
            .expect("daemon::tests::support::IpcTestServer::shutdown failed to signal shutdown");
        tokio::time::timeout(timeout, self.handle)
            .await
            .expect("daemon::tests::support::IpcTestServer::shutdown timed out")
            .expect("daemon::tests::support::IpcTestServer::shutdown server task failed");
        assert!(
            !self.path.exists(),
            "daemon::tests::support::IpcTestServer::shutdown expected socket path cleanup"
        );
        match self.previous_socket_path {
            Some(value) => std::env::set_var("BACKUP_SYNC_IPC_SOCKET", value),
            None => std::env::remove_var("BACKUP_SYNC_IPC_SOCKET"),
        }
    }
}

#[cfg(unix)]
/// Remove duplicated setup boilerplate across IPC test files.
pub async fn spawn_test_server(test_name: &str) -> IpcTestServer {
    let tmp = tempdir().expect("daemon::tests::support::spawn_test_server temp dir");
    std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o700))
        .expect("daemon::tests::support::spawn_test_server secure temp dir");
    let path = tmp.path().join("daemon-ipc.sock");
    let previous_socket_path = std::env::var_os("BACKUP_SYNC_IPC_SOCKET");
    std::env::set_var("BACKUP_SYNC_IPC_SOCKET", &path);
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
    let handle = spawn_server(
        shared,
        destinations,
        Duration::from_secs(5),
        runtime_defaults.ipc_request_max_bytes,
        runtime_defaults.ipc_read_chunk_bytes,
        10,
        shutdown_rx,
    )
    .await
    .unwrap_or_else(|error| {
        panic!("daemon::tests::support::spawn_test_server {test_name} failed: {error:#}")
    });
    assert_eq!(
        socket_path().expect("daemon::tests::support::spawn_test_server socket path"),
        path
    );
    IpcTestServer {
        _temp_dir: tmp,
        shutdown_tx,
        handle,
        path,
        previous_socket_path,
    }
}

#[cfg(unix)]
/// Avoid socket path collisions from parallel tests.
pub async fn ipc_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().await
}

#[cfg(unix)]
/// Improve determinism versus race-prone fixed delays.
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
/// Centralize deterministic request/response flow for IPC integration tests.
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
