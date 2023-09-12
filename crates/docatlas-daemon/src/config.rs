use std::path::{Path, PathBuf};

use clap::{Args, Parser};
use merge::Merge;
use serde::Deserialize;
use tracing::log::LevelFilter;

mod merge_strategies;

const DEFAULT_PATH: &str = "/var/lib/docatlas";
const DEFAULT_HOST: &str = "localhost";
const DEFAULT_PORT: u16 = 3676;
const DEFAULT_LOG_LEVEL_FILTER: LevelFilter = LevelFilter::Info;

/// The docatlas-daemon daemon configuration
#[derive(Debug, Default, Clone, Deserialize, Args, Merge)]
pub struct DaemonConfig {
    #[clap(long)]
    path: Option<PathBuf>,
    #[clap(long)]
    host: Option<String>,
    #[clap(long)]
    port: Option<u16>,

    #[clap(long = "log")]
    log_level: Option<LevelFilter>,
}

impl DaemonConfig {
    /// Gets the path to store all daemon data. By default this value is `"/var/lib/docatlas/"`.
    pub fn path(&self) -> &Path {
        self.path.as_deref().unwrap_or(Path::new(DEFAULT_PATH))
    }

    /// Gets the default host to open the server on. By default this value is `"localhost"`.
    pub fn host(&self) -> &str {
        self.host.as_deref().unwrap_or(DEFAULT_HOST)
    }

    /// Gets the port to open the daemon on. By default this value is `3676`.
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(DEFAULT_PORT)
    }

    /// Gets the log level. By default this value [`LevelFilter::Info`](LevelFilter::Info)
    pub fn log_level(&self) -> &LevelFilter {
        self.log_level.as_ref().unwrap_or(&DEFAULT_LOG_LEVEL_FILTER)
    }
}

#[derive(Debug, Parser)]
pub struct CliDaemonConfig {
    #[clap(flatten)]
    config: DaemonConfig,
}

impl CliDaemonConfig {
    pub fn config(&self) -> &DaemonConfig {
        &self.config
    }

    pub fn into_config(self) -> DaemonConfig {
        self.config
    }
}
