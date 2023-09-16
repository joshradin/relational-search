//! Transport is used for transporting data between two end points

use std::fmt::Debug;
use std::future::Future;
use std::io::{Error, ErrorKind};
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{io, iter};

use async_stream::stream;
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::{Sink, Stream, StreamExt, TryStreamExt};
use interprocess::local_socket::tokio::{
    LocalSocketStream, OwnedReadHalf as LocalOwnedReadHalf, OwnedWriteHalf as LocalOwnedWriteHalf,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use tokio::io::{
    AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, BufWriter, ReadBuf,
};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

pub mod packet_reader;
//
// #[async_trait]
// pub trait Transport<I: Send, O: Send>: Send {
//     type Error: 'static + Send;
//     type ItemSink: Sink<I>;
//     type OutItemStream: Stream<Item=O>;
//
//     async fn send(&mut self, input: I) -> Result<(), Self::Error>;
//     async fn receive(&mut self) -> Result<O, Self::Error>;
//
//     fn split(self) -> (Self::OutItemStream, Self::ItemSink);
// }
//
//
// pub trait TransportExt<Item: Send, OutItem: Send>: Transport<Item, OutItem> {
//
//     /// Modifies the input for this transport via function
//     fn with<U, E, Fut, F>(self, func: F) -> With<Self, Item, OutItem, U, E, Fut, F>
//     where
//         U: Send,
//         F: FnMut(U) -> Fut + Send,
//         Fut: Future<Output = Result<Item, E>> + Send,
//         E: From<Self::Error> + Send,
//         Self: Sized,
//     {
//         With {
//             func,
//             transport: self,
//             _kind: Default::default(),
//         }
//     }
//
//     /// Maps the output
//     fn then<R, Fut, F>(self, func: F) -> Then<Self, Item, OutItem, R, Fut, F>
//     where
//         R: Send,
//         F: FnMut(OutItem) -> Fut + Send,
//         Fut: Future<Output = Result<R, Self::Error>> + Send,
//         Self: Sized,
//     {
//         Then {
//             func,
//             transport: self,
//             _kind: Default::default(),
//         }
//     }
//
//     /// Maps the output
//     fn then_stream<R, Fut, F>(self, func: F) -> ThenStream<Self, Item, OutItem, R, Fut, F>
//     where
//         R: Send,
//         Fut: Future<Output = Result<R, Self::Error>> + Send,
//         F: FnMut(BoxStream<Result<OutItem, Self::Error>>) -> Fut + Send,
//         Self: Sized,
//     {
//         ThenStream {
//             func,
//             transport: self,
//             _kind: Default::default(),
//         }
//     }
// }
//
// impl<I: Send, O: Send, T: Transport<I, O>> TransportExt<I, O> for T {}
//
// #[derive(Debug)]
// pub struct With<T, Item, OutItem, U, E, Fut, F>
// where
//     T: Transport<Item, OutItem>,
//     F: FnMut(U) -> Fut + Send,
//     E: From<T::Error> + Send,
//     Fut: Future<Output = Result<Item, E>>,
//     Item: Send,
//     OutItem: Send,
//     U: Send,
// {
//     func: F,
//     transport: T,
//     _kind: PhantomData<(U, OutItem)>,
// }
//
// #[async_trait]
// impl<T, Item, OutItem, U, E, Fut, F> Transport<U, OutItem> for With<T, Item, OutItem, U, E, Fut, F>
// where
//     T: Transport<Item, OutItem>,
//     E: From<T::Error> + Send + 'static,
//     F: FnMut(U) -> Fut + Send,
//     Fut: Future<Output = Result<Item, E>> + Send,
//     Item: Send,
//     OutItem: Send,
//     U: Send,
// {
//     type Error = E;
//     type ItemSink = Pin<Box<dyn Sink<U, Error=E>>>;
//     type OutItemStream = T::OutItemStream;
//
//     async fn send(&mut self, input: U) -> Result<(), Self::Error> {
//         let converted: Item = (self.func)(input).await?;
//         self.transport.send(converted).await.map_err(|e| e.into())
//     }
//
//     async fn receive(&mut self) -> Result<OutItem, Self::Error> {
//         self.transport.receive().await.map_err(|e| e.into())
//     }
//
//     fn split(self) -> (Self::OutItemStream, Self::ItemSink) {
//         let (stream, sink) = self.transport.split();
//         (stream, sink)
//     }
// }
//
// #[derive(Debug)]
// pub struct Then<T, Item, OutItem, R, Fut, F>
// where
//     T: Transport<Item, OutItem>,
//     F: FnMut(OutItem) -> Fut,
//     Fut: Future<Output = Result<R, T::Error>>,
//     Item: Send,
//     OutItem: Send,
//     R: Send,
// {
//     func: F,
//     transport: T,
//     _kind: PhantomData<(OutItem, R, Item)>,
// }
//
// #[async_trait]
// impl<T, Item: Send, OutItem: Send, R: Send, Fut, F> Transport<Item, R>
//     for Then<T, Item, OutItem, R, Fut, F>
// where
//     T: Transport<Item, OutItem> + Send,
//     F: FnMut(OutItem) -> Fut + Send,
//     Fut: Future<Output = Result<R, T::Error>> + Send,
// {
//     type Error = T::Error;
//     type ItemSink = T::ItemSink;
//     type OutItemStream = Pin<Box<dyn Stream<Item=R>>>;
//
//     async fn send(&mut self, input: Item) -> Result<(), Self::Error> {
//         self.transport.send(input).await
//     }
//
//     async fn receive(&mut self) -> Result<R, Self::Error> {
//         let received = self.transport.receive().await?;
//         (self.func)(received).await
//     }
//
//     fn split(self) -> (Self::OutItemStream, Self::ItemSink) {
//         self.split()
//     }
// }
//
// #[derive(Debug)]
// pub struct ThenStream<T, Item, OutItem, R, Fut, F>
// where
//     T: Transport<Item, OutItem>,
//     Fut: Future<Output = Result<R, T::Error>> + Send,
//     Item: Send,
//     OutItem: Send,
//     R: Send,
//     F: FnMut(BoxStream<Result<OutItem, T::Error>>) -> Fut + Send,
// {
//     func: F,
//     transport: T,
//     _kind: PhantomData<(OutItem, R, Item)>,
// }
//
// #[async_trait]
// impl<T, Item, OutItem, R, Fut, F> Transport<Item, R>
//     for ThenStream<T, Item,OutItem, R, Fut, F>
// where
//     T: Transport<Item, OutItem>,
//     Fut: Future<Output = Result<R, T::Error>> + Send,
//     Item: Send,
//     OutItem: Send,
//     R: Send,
//     F: FnMut(BoxStream<Result<OutItem, T::Error>>) -> Fut + Send,
// {
//     type Error = T::Error;
//     type ItemSink = T::ItemSink;
//     type OutItemStream = Pin<Box<dyn Stream<Item=R>>>;
//
//     async fn send(&mut self, input: Item) -> Result<(), Self::Error> {
//         self.transport.send(input).await
//     }
//
//     async fn receive(&mut self) -> Result<R, Self::Error> {
//         let stream = stream! {
//             loop {
//                 yield self.transport.receive().await;
//             }
//         }.boxed();
//         let x = (self.func)(stream).await;
//         x
//     }
//
//     fn split(self) -> (Self::OutItemStream, Self::ItemSink) {
//         todo!()
//     }
// }
//
// #[derive(Debug)]
// pub struct TransportImpl<R: AsyncRead, W: AsyncWrite> {
//     read: BufReader<R>,
//     write: BufWriter<W>,
//     buffer: Box<[u8]>,
//     filled: usize,
//     index: usize,
// }
//
// const BUFFER_SIZE: usize = 512;
//
// #[async_trait]
// impl<R: AsyncRead + Unpin + Send, W: AsyncWrite + Unpin + Send> Transport<Vec<u8>, u8>
//     for TransportImpl<R, W>
// {
//     type Error = io::Error;
//     type ItemSink = ();
//     type OutItemStream = ();
//
//     async fn send(&mut self, input: Vec<u8>) -> Result<(), Self::Error> {
//         self.write.write_all(input.as_slice()).await
//     }
//
//     async fn receive(&mut self) -> Result<u8, Self::Error> {
//         if self.index == self.filled {
//             self.index = 0;
//             self.filled = self.read.read(&mut self.buffer).await?;
//             if self.filled == 0 && self.buffer.len() != 0 {
//                 return Err(io::Error::new(
//                     ErrorKind::UnexpectedEof,
//                     "reach end of stream",
//                 ));
//             }
//         }
//
//         let emit = self.buffer[self.index];
//         self.index += 1;
//
//         Ok(emit)
//     }
//
//     fn split(self) -> (Self::OutItemStream, Self::ItemSink) {
//         todo!()
//     }
// }
//
// /// A transport built on a tcp stream
// #[derive(Debug)]
// pub struct TcpTransport {
//     local_addr: Option<SocketAddr>,
//     transport_impl: TransportImpl<OwnedReadHalf, OwnedWriteHalf>,
// }
// impl TcpTransport {
//     pub fn new(stream: TcpStream, buffer_capacity: impl Into<Option<usize>>) -> Self {
//         let local_addr = stream.local_addr().ok();
//         let (read, write) = stream.into_split();
//         let buffer = iter::repeat(0)
//             .take(buffer_capacity.into().unwrap_or(BUFFER_SIZE))
//             .collect::<Vec<_>>()
//             .into_boxed_slice();
//         Self {
//             local_addr,
//             transport_impl: TransportImpl {
//                 read: BufReader::new(read),
//                 write: BufWriter::new(write),
//                 buffer,
//                 filled: 0,
//                 index: 0,
//             },
//         }
//     }
//
//     pub fn local_addr(&self) -> Option<&SocketAddr> {
//         self.local_addr.as_ref()
//     }
// }
//
// #[async_trait]
// impl Transport<Vec<u8>, u8> for TcpTransport {
//     type Error = io::Error;
//
//     async fn send(&mut self, input: Vec<u8>) -> Result<(), Self::Error> {
//         self.transport_impl.send(input).await
//     }
//
//     async fn receive(&mut self) -> Result<u8, Self::Error> {
//         self.transport_impl.receive().await
//     }
// }
//
// #[derive(Debug)]
// struct WriteWrapper<W: futures::io::AsyncWrite> {
//     writer: W,
// }
//
// impl<W: futures::io::AsyncWrite> From<W> for WriteWrapper<W> {
//     fn from(value: W) -> Self {
//         Self { writer: value }
//     }
// }
//
// impl<W: futures::io::AsyncWrite + Unpin> AsyncWrite for WriteWrapper<W> {
//     fn poll_write(
//         mut self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         buf: &[u8],
//     ) -> Poll<Result<usize, Error>> {
//         Pin::new(&mut self.writer).poll_write(cx, buf)
//     }
//
//     fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
//         Pin::new(&mut self.writer).poll_flush(cx)
//     }
//
//     fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
//         Pin::new(&mut self.writer).poll_close(cx)
//     }
// }
//
// #[derive(Debug)]
// struct ReadWrapper<R: futures::io::AsyncRead> {
//     reader: R,
// }
//
// impl<R: futures::io::AsyncRead + Unpin> AsyncRead for ReadWrapper<R> {
//     fn poll_read(
//         mut self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         buf: &mut ReadBuf<'_>,
//     ) -> Poll<io::Result<()>> {
//         Pin::new(&mut self.reader)
//             .poll_read(cx, buf.initialize_unfilled())
//             .map_ok(|result| buf.set_filled(result))
//     }
// }
//
// impl<R: futures::io::AsyncRead> From<R> for ReadWrapper<R> {
//     fn from(value: R) -> Self {
//         Self { reader: value }
//     }
// }
//
// /// A transport built on a local interprocess socket
// #[derive(Debug)]
// pub struct LocalSocketTransport {
//     peer_id: io::Result<u32>,
//     transport_impl:
//         TransportImpl<ReadWrapper<LocalOwnedReadHalf>, WriteWrapper<LocalOwnedWriteHalf>>,
// }
//
// #[async_trait]
// impl Transport<Vec<u8>, u8> for LocalSocketTransport {
//     type Error = io::Error;
//
//     async fn send(&mut self, input: Vec<u8>) -> Result<(), Self::Error> {
//         self.transport_impl.send(input).await
//     }
//
//     async fn receive(&mut self) -> Result<u8, Self::Error> {
//         self.transport_impl.receive().await
//     }
// }
//
// impl LocalSocketTransport {
//     pub fn new(stream: LocalSocketStream, buffer_capacity: impl Into<Option<usize>>) -> Self {
//         let peer_id = stream.peer_pid();
//         let (read, write) = stream.into_split();
//         let buffer = iter::repeat(0)
//             .take(buffer_capacity.into().unwrap_or(BUFFER_SIZE))
//             .collect::<Vec<_>>()
//             .into_boxed_slice();
//         Self {
//             peer_id,
//             transport_impl: TransportImpl {
//                 read: BufReader::new(read.into()),
//                 write: BufWriter::new(write.into()),
//                 buffer,
//                 filled: 0,
//                 index: 0,
//             },
//         }
//     }
//
//     pub fn peer_id(&self) -> Option<&u32> {
//         self.peer_id.as_ref().ok()
//     }
// }
//
// /// Transfers serializable data via packets
// pub struct PacketTransport<T: Transport<Vec<u8>, u8>, Item: Serialize, OutItem: DeserializeOwned> {
//     wrapped: T,
//     _kinds: PhantomData<(Item, OutItem)>
// }
//
// // impl<Item: Serialize + Send + 'static, OutItem: DeserializeOwned + Send +'static> PacketTransport<Item, OutItem> {
// //
// // }
//
// struct Packet<T> {
//     data: T
// }
//
// impl<T> Packet<T> {
//     async fn from_stream<S : Stream<Item=u8> + Unpin>(mut stream: S) -> Result<Self, TransportError> {
//         let mut buffer = [0u8; 8];
//         let filled = fill_buffer(&mut stream, &mut buffer).await;
//
//
//         todo!()
//     }
// }
//
// async fn fill_buffer<S : Stream<Item=u8> + Unpin>(mut stream: &mut S, buffer: &mut [u8]) -> usize {
//     let mut filled = 0;
//     for byte in buffer.iter_mut() {
//         match stream.next().await {
//             None => {
//                 break;
//             }
//             Some(b) => {
//                 *byte = b;
//                 filled += 1;
//             }
//         }
//     }
//     filled
// }
//
// /// An error occurred during transport
// #[derive(Debug, Error)]
// pub enum TransportError {
//     #[error(transparent)]
//     RonError(#[from] ron::Error),
//     #[error(transparent)]
//     IoError(#[from] io::Error)
// }
