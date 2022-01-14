// use std::pin::Pin;

// use anyhow::{bail, Result};
// use bytes::{BufMut, BytesMut};
// use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader, Split};

// pub trait Stream<T: Box<dyn AsyncRead + AsyncWrite + Send + Sync + Sized>> {
//     fn next_line(&mut self) -> Result<Vec<u8>>;
// }

// pub struct MyStream<T: AsyncRead + AsyncWrite + Send + Sync + Sized> {
//     read: Split<BufReader<tokio::io::ReadHalf<T>>>,
//     write: tokio::io::WriteHalf<T>,
// }

// impl<T: AsyncRead + AsyncWrite + Send + Sync + Sized> MyStream<T> {
//     pub fn new(stream: T) -> Result<Box<dyn Stream<T>>> {
//         let (read, write) = tokio::io::split(stream);
//         let read = BufReader::new(read);
//         let read = read.split(b'\n');

//         Ok(Box::new(MyStream { read, write }))
//     }
// }

// impl<T: AsyncRead + AsyncWrite + Send + Sync + Sized> Stream<T> for MyStream<T> {
//     async fn next_line(&mut self) -> Result<Vec<u8>> {
//         //let mut buffer: BytesMut = BytesMut::new();
//         if let Some(buffer) = self.read.next_segment().await? {
//             Ok(buffer)
//         } else {
//             bail!("读取失败");
//         }
//     }
// }
