use tokio::signal;
use tracing::warn;

/// Purpose: Waits for a shutdown signal such as Ctrl C.
///
/// Inputs: none.
/// Outputs: `()` after the signal is observed or an error is logged.
/// Ties to: daemon shutdown handling.
/// Side effects: Registers signal listeners and awaits signal streams.
/// Why: provide a unified shutdown hook for the daemon.
pub async fn shutdown_signal() {
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
