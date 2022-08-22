use std::{
    ffi::OsStr,
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use tokio::{
    io::{
        AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt,
        BufReader, ReadBuf,
    },
    net::{TcpStream, ToSocketAddrs},
    time,
};

use crate::utils::{Interactive, RecvUntil};

use super::ProcessTube;

/// A wrapper to provide extra methods. Note that the API from this crate is different from pwntools.
pub struct Tube<T: Unpin> {
    inner: BufReader<T>,
    /// This field is only used by methods directly provided by this struct and not methods from traits.
    pub timeout: Duration,
}

const NEW_LINE: u8 = 0xA;

impl<T: AsyncRead + AsyncWrite + Unpin> Tube<T> {
    /// Construct a new `Tube<T>`.
    pub fn new(inner: T) -> Self {
        Self {
            inner: BufReader::new(inner),
            timeout: Duration::MAX,
        }
    }

    /// Construct a new `Tube<T>` with the supplied timeout argument. Note that timeout is only
    /// implemented for methods directly provided by this struct and not methods from traits.
    ///
    /// Equivlently:
    /// ```rust, no_run
    /// use io_tubes::tubes::Tube;
    /// use std::time::Duration;
    ///
    /// let mut p = Tube::process("/usr/bin/cat").unwrap();
    /// p.timeout = Duration::from_millis(50);
    /// ```
    pub fn with_timeout(inner: T, timeout: Duration) -> Self {
        Self {
            inner: BufReader::new(inner),
            timeout,
        }
    }

    /// Receive up to `len` bytes.
    pub async fn recv(&mut self, len: usize) -> io::Result<Vec<u8>> {
        let mut buf = vec![0; len];
        let len = time::timeout(self.timeout, self.read(&mut buf[..]))
            .await
            .unwrap_or(Ok(0))?;
        buf.truncate(len);
        Ok(buf)
    }

    /// Receive until new line (0xA byte) is reached or EOF is reached.
    pub async fn recv_line(&mut self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        time::timeout(self.timeout, self.read_until(NEW_LINE, &mut buf))
            .await
            .unwrap_or(Ok(0))?;
        Ok(buf)
    }

    /// Receive until the delims are found or EOF is reached.
    ///
    /// A lookup table will be built to enable efficient matching of long patterns.
    pub async fn recv_until(&mut self, delims: &[u8]) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        time::timeout(self.timeout, RecvUntil::new(self, delims, &mut buf))
            .await
            .unwrap_or(Ok(()))?;
        Ok(buf)
    }

    /// Send data and flush.
    pub async fn send(&mut self, data: &[u8]) -> io::Result<()> {
        self.write_all(data).await?;
        self.flush().await
    }

    /// Same as send, but add new line (0xA byte).
    pub async fn send_line(&mut self, data: &[u8]) -> io::Result<()> {
        self.write_all(data).await?;
        self.write_all(&[NEW_LINE]).await?;
        self.flush().await
    }

    /// Send line after receiving the pattern from read.
    pub async fn send_line_after(&mut self, pattern: &[u8], data: &[u8]) -> io::Result<Vec<u8>> {
        let result = self.recv_until(pattern).await?;
        self.send_line(data).await?;
        Ok(result)
    }

    /// Connect the tube to stdin and stdout so you can interact with it directly.
    pub async fn interactive(&mut self) -> io::Result<()> {
        Interactive::new(self).await
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

impl<T: AsyncRead + Unpin> AsyncRead for Tube<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncWrite for Tube<T> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

impl<T: AsyncRead + Unpin> AsyncBufRead for Tube<T> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<&[u8]>> {
        Pin::new(&mut self.get_mut().inner).poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        Pin::new(&mut self.get_mut().inner).consume(amt)
    }
}
