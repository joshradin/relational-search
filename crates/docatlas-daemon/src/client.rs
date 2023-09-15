use async_stream::stream;
use futures::stream::BoxStream;
use futures::{FutureExt, Sink, SinkExt, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_pickle::DeOptions;
use tokio::io::{AsyncRead, AsyncReadExt};

pub struct Client<Si, So>
where
    Si: Stream<Item = ClientRequest> + Unpin,
    So: Sink<ClientResponse> + Unpin,
{
    stream: Si,
    sink: So,
}

impl<Si, So> Client<Si, So>
where
    Si: Stream<Item = ClientRequest> + Unpin,
    So: Sink<ClientResponse> + Unpin,
{
    pub fn new(stream: Si, sink: So) -> Self {
        Self { stream, sink }
    }

    pub async fn poll_request(&mut self) -> Option<ClientRequest> {
        self.stream.next().await
    }

    pub async fn send_response(&mut self, resp: ClientResponse) -> Result<(), So::Error> {
        self.sink.send(resp).await
    }
}

pub fn wrap_async_read<R: AsyncRead + Unpin + Send + 'static>(
    mut read: R,
) -> BoxStream<'static, ClientRequest> {
    (stream! {
        loop {
            yield read_packet(&mut read).await;
        }
    })
    .boxed()
}

async fn read_packet<R: AsyncRead + Unpin>(reader: &mut R) -> ClientRequest {
    let len = reader
        .read_u64()
        .await
        .expect("could not read packet length");
    let mut buffer = vec![0_u8; len as usize];
    reader
        .read_exact(&mut buffer)
        .await
        .expect("could not fill buffer");
    serde_pickle::from_slice(&buffer, DeOptions::new()).expect("could not deserialize packet")
}

/// A request *received* from a client connection
#[derive(Debug, Deserialize)]
pub struct ClientRequest {}

/// A response *sent* to a client as a response to a request
#[derive(Debug, Serialize)]
pub struct ClientResponse {}
