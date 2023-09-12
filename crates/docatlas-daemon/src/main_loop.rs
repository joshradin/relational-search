//! Contains the main loop

use futures::StreamExt;
use log::info;
use tokio::net::TcpListener;
use crate::client;
use crate::client::Client;

use crate::config::DaemonConfig;
use crate::error::DaemonError;

pub async fn main_loop(config: &DaemonConfig) -> Result<(), DaemonError> {
    let listener = TcpListener::bind((config.host(), config.port())).await?;

    while let Ok((mut stream, socket)) = listener.accept().await {
        tokio::spawn(async move {
            info!("new client connected at {socket}");
            let (stream, sink) = stream.into_split();
            let mut stream = client::wrap_async_read(stream);
            let x = stream.next().await;
            // let client = Client::new(stream, sink);

        });
    }
    Ok(())
}
