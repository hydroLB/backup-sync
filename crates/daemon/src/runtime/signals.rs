use tokio::signal;
use tracing::warn;

/// Summary: Waits for a shutdown signal such as Ctrl C.
///
/// Inputs: none.
///
/// Outputs: `()` after the signal is observed or an error is logged.
///
/// Side effects: Registers signal listeners and awaits signal streams.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon shutdown handling.
///
/// Why this exists: provide a unified shutdown hook for the daemon.
pub(crate) async fn shutdown_signal() {
    #[cfg(unix)]
    let mut term = match signal::unix::signal(signal::unix::SignalKind::terminate()) {
        Ok(stream) => Some(stream),
        Err(e) => {
            warn!(
                "daemon::runtime::shutdown_signal failed to listen for SIGTERM: {}",
                e
            );
            None
        }
    };

    #[cfg(unix)]
    let mut interrupt = match signal::unix::signal(signal::unix::SignalKind::interrupt()) {
        Ok(stream) => Some(stream),
        Err(e) => {
            warn!(
                "daemon::runtime::shutdown_signal failed to listen for SIGINT: {}",
                e
            );
            None
        }
    };

    #[cfg(unix)]
    {
        tokio::select! {
            _ = signal::ctrl_c() => {}
            _ = async { if let Some(ref mut stream) = term { stream.recv().await; } } => {}
            _ = async { if let Some(ref mut stream) = interrupt { stream.recv().await; } } => {}
        }
    }

    #[cfg(not(unix))]
    {
        if let Err(e) = signal::ctrl_c().await {
            warn!(
                "daemon::runtime::shutdown_signal failed to listen for ctrl_c: {}",
                e
            );
        }
    }
}
