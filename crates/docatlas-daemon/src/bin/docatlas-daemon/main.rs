use clap::Parser;
use docatlas_daemon::config::{CliDaemonConfig, DaemonConfig};
use docatlas_daemon::main_loop::main_loop;
use futures::{FutureExt, StreamExt};
use log::debug;
use merge::Merge;
use std::fs::File;
use tracing::info;

mod logging;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let mut config = CliDaemonConfig::parse().into_config();
    if let Ok(file) = File::open(config.path().join("config.yaml")) {
        let read_config: DaemonConfig =
            serde_yaml::from_reader(file).expect("could not parse config file");
        config.merge(read_config);
    }
    std::fs::create_dir_all(config.path())?;

    logging::setup_logging(&config);
    info!(
        "starting docatlasd instance at {:?} on port {}.",
        config.host(),
        config.port()
    );
    info!("docatlasd version: {}", env!("CARGO_PKG_VERSION"));
    debug!("running in dir {:?}", config.path());

    main_loop(&config).await?;
    Ok(())
}
