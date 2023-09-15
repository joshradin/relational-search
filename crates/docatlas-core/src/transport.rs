//! Transport is used for transporting data between two end points

use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::net::SocketAddr;

use async_trait::async_trait;
use futures::Sink;
use static_assertions::assert_obj_safe;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter, ReadHalf};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

#[async_trait]
pub trait Transport<I: Send, O: Send> {
    type Error: 'static + Send;

    async fn send(&mut self, input: I) -> Result<(), Self::Error>;
    async fn receive(&mut self) -> Result<O, Self::Error>;
}

assert_obj_safe!(Transport<u8, u8, Error=()>);

pub trait TransportExt<Item: Send, OutItem: Send>: Transport<Item, OutItem> {
    fn with<U, Fut, F>(self, func: F) -> With<Self, Item, OutItem, U, Fut, F>
    where
        U: Send,
        F: FnMut(U) -> Fut,
        Fut: Future<Output = Result<Item, Self::Error>>,
        Self: Sized,
    {
        With {
            func,
            transport: self,
            _kind: Default::default(),
        }
    }

    fn then<R, Fut, F>(self, func: F) -> Then<Self, Item, OutItem, R, Fut, F>
        where
            R: Send,
            F: FnMut(OutItem) -> Fut,
            Fut: Future<Output = Result<R, Self::Error>>,
            Self: Sized,
    {
        Then {
            func,
            transport: self,
            _kind: Default::default(),
        }
    }
}

impl<I: Send, O: Send, T: TransportExt<I, O>> TransportExt<I, O> for T {}

#[derive(Debug)]
pub struct With<T, Item: Send, OutItem: Send, U: Send, Fut, F>
where
    T: Transport<Item, OutItem>,
    F: FnMut(U) -> Fut,
    Fut: Future<Output = Result<Item, T::Error>>,
{
    func: F,
    transport: T,
    _kind: PhantomData<(U, OutItem)>,
}

#[async_trait]
impl<T, Item: Send, OutItem: Send, U: Send, Fut, F> Transport<U, OutItem> for With<T, Item, OutItem, U, Fut, F>
where
    T: Transport<Item, OutItem> + Send,
    F: FnMut(U) -> Fut + Send,
    Fut: Future<Output = Result<Item, T::Error>> + Send,
{
    type Error = T::Error;

    async fn send(&mut self, input: U) -> Result<(), Self::Error> {
        let converted: Item = (self.func)(input).await?;
        self.transport.send(converted).await
    }

    async fn receive(&mut self) -> Result<OutItem, Self::Error> {
        self.transport.receive().await
    }
}

#[derive(Debug)]
pub struct Then<T, Item: Send, OutItem: Send, R: Send, Fut, F>
    where
        T: Transport<Item, OutItem>,
        F: FnMut(OutItem) -> Fut,
        Fut: Future<Output = Result<R, T::Error>>,
{
    func: F,
    transport: T,
    _kind: PhantomData<(OutItem, R, Item)>,
}

#[async_trait]
impl<T, Item: Send, OutItem: Send, R: Send, Fut, F> Transport<Item, R> for Then<T, Item, OutItem, R, Fut, F>
    where
        T: Transport<Item, OutItem> + Send,
        F: FnMut(OutItem) -> Fut + Send,
        Fut: Future<Output = Result<R, T::Error>> + Send,
{
    type Error = T::Error;

    async fn send(&mut self, input: Item) -> Result<(), Self::Error> {
        self.transport.send(input).await
    }

    async fn receive(&mut self) -> Result<R, Self::Error> {
        let received = self.transport.receive().await?;
        (self.func)(received).await
    }
}

#[derive(Debug)]
pub struct TcpTransport {
    local_addr: Option<SocketAddr>,
    read: BufReader<OwnedReadHalf>,
    write: BufWriter<OwnedWriteHalf>
}

impl TcpTransport {
    pub fn new(stream: TcpStream) -> Self {
        let local_addr = stream.local_addr().ok();
        let (read, write) = stream.into_split();
        Self { local_addr, read: BufReader::new(read), write: BufWriter::new(write) }
    }

    pub fn local_addr(&self) -> Option<&SocketAddr> {
        self.local_addr.as_ref()
    }
}

#[async_trait]
impl Transport<Vec<u8>, u8> for TcpTransport {
    type Error = io::Error;

    async fn send(&mut self, input: Vec<u8>) -> Result<(), Self::Error> {
        self.write.write_all(input.as_slice()).await
    }

    async fn receive(&mut self) -> Result<u8, Self::Error> {
        let mut buf = [0u8];
        self.read.read_exact(&mut buf).await?;
        Ok(buf[0])
    }
}