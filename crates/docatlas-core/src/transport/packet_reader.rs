//! Async packet reader

use std::io;
use std::marker::PhantomData;
use std::pin::{Pin, pin};
use std::task::{Context, Poll};
use futures::{AsyncWrite, Stream};
use std::future::Future;
use async_stream::stream;

use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncReadExt, BufReader};

#[derive(Debug)]
pub struct Packet<T> {
    wrapped: T,
}

#[derive(Debug)]
pub struct PacketReader<T: DeserializeOwned, R: AsyncRead + Unpin> {
    reader: BufReader<R>,
    _emit: PhantomData<T>,
}

impl<T: DeserializeOwned, R: AsyncRead + Unpin> PacketReader<T, R> {
    pub fn stream<'a>(&'a mut self) -> impl Stream<Item=Result<Packet<T>, PacketReadError>> + 'a {
        stream! {
            let last = loop {
                let r = self.next().await;
                match r {
                    Ok(r) => yield Ok(r),
                    Err(e) => break Err(e)
                }
            };
            yield last;
        }
    }
}


impl<T: DeserializeOwned, R: AsyncRead + Unpin> PacketReader<T, R> {
    async fn next(&mut self) -> Result<Packet<T>, PacketReadError> {
        let len = self.reader.read_u64().await?;
        let mut buffer = vec![0_u8; len as usize - 8];
        self.reader.read_exact(&mut buffer).await?;
        let read: T = ron::de::from_bytes(&buffer).map_err(|e| e.code)?;
        Ok(Packet { wrapped: read })
    }
}



#[derive(Debug, thiserror::Error)]
pub enum PacketReadError {
    #[error(transparent)]
    RonError(#[from] ron::Error),
    #[error(transparent)]
    IoError(#[from] io::Error),
}
