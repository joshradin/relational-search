use std::io;

/// An error occurred in the daemon
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error(transparent)]
    IoError(#[from] io::Error),
}
