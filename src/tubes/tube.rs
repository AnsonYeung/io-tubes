use std::{
    ffi::OsStr,
    io,
    pin::Pin,
    task::{Context, Poll},
};

use pin_project_lite::pin_project;
use tokio::{
    io::{AsyncBufRead, AsyncRead, AsyncWrite, BufReader, ReadBuf},
    net::{TcpStream, ToSocketAddrs},
};

use crate::utils::Interactive;

use super::ProcessTube;

pin_project! {
    pub struct Tube<T> {
        #[pin]
        inner: BufReader<T>,
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> Tube<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: BufReader::new(inner),
        }
    }

    /// ```rust, ignore
    /// async pub fn interactive(&mut self) -> io::Result<()>
    /// ```
    pub fn interactive(&mut self) -> Interactive<T> {
        Interactive::new(self)
    }
}

impl Tube<ProcessTube> {
    pub fn process<S: AsRef<OsStr>>(program: S) -> io::Result<Self> {
        Ok(Self::new(ProcessTube::new(program)?))
    }
}

impl Tube<TcpStream> {
    pub async fn remote<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        Ok(Self::new(TcpStream::connect(addr).await?))
    }
}

impl<T: AsyncRead> AsyncRead for Tube<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        self.project().inner.poll_read(cx, buf)
    }
}

impl<T: AsyncRead + AsyncWrite> AsyncWrite for Tube<T> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.project().inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        self.project().inner.poll_shutdown(cx)
    }
}

impl<T: AsyncRead> AsyncBufRead for Tube<T> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<&[u8]>> {
        self.project().inner.poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        self.project().inner.consume(amt)
    }
}
