//! Setups logging

use std::time::SystemTime;

use docatlas_daemon::config::DaemonConfig;

/// Setups logging using the [`fern`](fern) framework. Panics if fern
/// could not be initialized correctly.
pub fn setup_logging(config: &DaemonConfig) {
    fern::Dispatch::new()
        .format(|out, msg, record| {
            out.finish(format_args!(
                "{} [{}] {} - {}",
                humantime::format_rfc3339(SystemTime::now()),
                record.level(),
                record.module_path().unwrap_or("<unknown>"),
                msg
            ))
        })
        .level(config.log_level().clone())
        .chain(std::io::stdout())
        .chain(fern::log_file(config.path().join("docatlas.log")).unwrap())
        .apply()
        .expect("could not initialize logger")
}
