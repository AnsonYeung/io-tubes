use std::{
    ffi::OsStr,
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use log::debug;
use pretty_hex::PrettyHex;
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
#[derive(Debug)]
pub struct Tube<T>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
{
    /// The inner struct, usually a BufReader containing the original struct.
    pub inner: T,

    /// This field is only used by methods directly provided by this struct and not methods from
    /// traits like [`AsyncRead`].
    ///
    /// This is due to the fact that during the polling, there is no way to keep track of the
    /// futures involved. If 2 calls to the poll functions occurs, there is not enough
    /// information in the argument to deduce whether it come from the same future or the previous
    /// future is dropped and another future has started polling. As a result, the API will be
    /// producing inconsistent timeout if it is implemented.
    ///
    /// Luckily, [`tokio::time::timeout`] provides an easy way to add timeout to a future (which is
    /// how timeout is implemented in this library) so you can still have timeout behaviour on
    /// functions that doesn't support them.
    ///
    /// Hence, timeout can only be reliably implemented for async fn (which returns a future under
    /// the hood) or fn that return a future.
    pub timeout: Duration,

    read_buf_logged: usize,
}

const NEW_LINE: u8 = 0xA;

impl<T> Tube<BufReader<T>>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    /// Construct a new `Tube<T>`.
    pub fn new(inner: T) -> Self {
        Self {
            inner: BufReader::new(inner),
            timeout: Duration::MAX,
            read_buf_logged: 0,
        }
    }

    /// Construct a new `Tube<T>` with the supplied timeout argument. Note that timeout is only
    /// implemented for methods directly provided by this struct and not methods from traits.
    ///
    /// ```rust
    /// use io_tubes::tubes::{ProcessTube, Tube};
    /// use std::{io, time::Duration};
    ///
    /// #[tokio::main]
    /// async fn create_with_timeout() -> io::Result<()> {
    ///     let mut p = Tube::process("/usr/bin/cat")?;
    ///     p.timeout = Duration::from_millis(50);
    ///     // Equivalent to
    ///     let mut p =
    ///         Tube::with_timeout(ProcessTube::new("/usr/bin/cat")?, Duration::from_millis(50));
    ///     Ok(())
    /// }
    ///
    /// create_with_timeout();
    /// ```
    pub fn with_timeout(inner: T, timeout: Duration) -> Self {
        Self {
            inner: BufReader::new(inner),
            timeout,
            read_buf_logged: 0,
        }
    }
}

impl Tube<BufReader<ProcessTube>> {
    /// Create a process with supplied path to program.
    /// ```rust
    /// use io_tubes::tubes::Tube;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn create_process() -> io::Result<()> {
    ///     let mut p = Tube::process("/usr/bin/cat")?;
    ///     p.send("abcdHi!").await?;
    ///     let result = p.recv_until("Hi").await?;
    ///     assert_eq!(result, b"abcdHi");
    ///     Ok(())
    /// }
    ///
    /// create_process();
    /// ```
    pub fn process<S: AsRef<OsStr>>(program: S) -> io::Result<Self> {
        Ok(Self::new(ProcessTube::new(program)?))
    }
}

impl Tube<BufReader<TcpStream>> {
    /// Create a tube by connecting to the remote address.
    /// ```rust
    /// use io_tubes::tubes::{Listener, Tube};
    /// use std::{
    ///     io,
    ///     net::{IpAddr, Ipv4Addr, SocketAddr},
    /// };
    ///
    /// #[tokio::main]
    /// async fn create_remote() -> io::Result<()> {
    ///     let l = Listener::listen().await?;
    ///     let mut p =
    ///         Tube::remote(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), l.port()?)).await?;
    ///     let mut server = l.accept().await?;
    ///     p.send("Client Hello").await?;
    ///     server.send("Server Hello").await?;
    ///     assert_eq!(p.recv_until("Hello").await?, b"Server Hello");
    ///     assert_eq!(server.recv_until("Hello").await?, b"Client Hello");
    ///     Ok(())
    /// }
    ///
    /// create_remote();
    /// ```
    pub async fn remote(addr: impl ToSocketAddrs) -> io::Result<Self> {
        Ok(Self::new(TcpStream::connect(addr).await?))
    }
}

impl<T> Tube<T>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
{
    /// Construct a tube from any custom buffered type.
    pub fn from_buffered(inner: T) -> Self {
        Self {
            inner,
            timeout: Duration::MAX,
            read_buf_logged: 0,
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
    pub async fn recv_until(&mut self, delims: impl AsRef<[u8]>) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        time::timeout(
            self.timeout,
            RecvUntil::new(self, delims.as_ref(), &mut buf),
        )
        .await
        .unwrap_or(Ok(()))?;
        Ok(buf)
    }

    /// Send data and flush.
    pub async fn send(&mut self, data: impl AsRef<[u8]>) -> io::Result<()> {
        self.write_all(data.as_ref()).await?;
        self.flush().await
    }

    /// Same as send, but add new line (0xA byte).
    pub async fn send_line(&mut self, data: impl AsRef<[u8]>) -> io::Result<()> {
        self.write_all(data.as_ref()).await?;
        self.write_all(&[NEW_LINE]).await?;
        self.flush().await
    }

    /// Send line after receiving the pattern from read.
    /// ```rust
    /// use io_tubes::tubes::Tube;
    /// use std::io;
    ///
    /// #[tokio::main]
    /// async fn send_line_after() -> io::Result<()> {
    ///     let mut p = Tube::process("/usr/bin/cat")?;
    ///
    ///     p.send("Hello, what's your name? ").await?;
    ///     assert_eq!(
    ///         p.send_line_after("name", "test").await?,
    ///         b"Hello, what's your name"
    ///     );
    ///     assert_eq!(p.recv_line().await?, b"? test\n");
    ///
    ///     Ok(())
    /// }
    ///
    /// send_line_after();
    /// ```
    pub async fn send_line_after(
        &mut self,
        pattern: impl AsRef<[u8]>,
        data: impl AsRef<[u8]>,
    ) -> io::Result<Vec<u8>> {
        let result = self.recv_until(pattern).await?;
        self.send_line(data).await?;
        Ok(result)
    }

    /// Connect the tube to stdin and stdout so you can interact with it directly.
    pub async fn interactive(&mut self) -> io::Result<()> {
        Interactive::new(self).await
    }

    /// Consume the tube to get back the underlying BufReader
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> AsyncRead for Tube<T>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        let olen = buf.filled().len();

        if Pin::new(&mut self.get_mut().inner)
            .poll_read(cx, buf)?
            .is_pending()
        {
            return Poll::Pending;
        }

        debug!(target: "Tube::recv", "Received {:?}", buf.filled()[olen..].hex_dump());

        Poll::Ready(Ok(()))
    }
}

impl<T> AsyncWrite for Tube<T>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
{
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        let numb = match Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)? {
            Poll::Ready(numb) => numb,
            Poll::Pending => return Poll::Pending,
        };

        debug!(target: "Tube::send", "Sent {:?}", buf[..numb].hex_dump());

        Poll::Ready(Ok(numb))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context,
        bufs: &[io::IoSlice],
    ) -> Poll<io::Result<usize>> {
        let numb = match Pin::new(&mut self.get_mut().inner).poll_write_vectored(cx, bufs)? {
            Poll::Ready(numb) => numb,
            Poll::Pending => return Poll::Pending,
        };

        let mut to_log = numb;
        for buf in bufs {
            if to_log == 0 {
                break;
            }
            debug!(target: "Tube::send", "Send {:?}", buf[..to_log].hex_dump());
            to_log = to_log.saturating_sub(buf.len());
        }

        Poll::Ready(Ok(numb))
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}

impl<T> AsyncBufRead for Tube<T>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<&[u8]>> {
        let Self {
            inner,
            timeout: _,
            read_buf_logged,
        } = self.get_mut();

        let buf = match Pin::new(inner).poll_fill_buf(cx)? {
            Poll::Ready(buf) => buf,
            Poll::Pending => return Poll::Pending,
        };

        if buf.len() > *read_buf_logged {
            debug!(target: "Tube::recv", "Recevied {:?}", buf[*read_buf_logged..].hex_dump());
            *read_buf_logged = buf.len();
        }

        Poll::Ready(Ok(buf))
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        self.read_buf_logged -= amt;
        Pin::new(&mut self.get_mut().inner).consume(amt);
    }
}

impl<T> From<Tube<BufReader<T>>> for BufReader<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn from(tube: Tube<BufReader<T>>) -> Self {
        tube.into_inner()
    }
}

impl<T> From<T> for Tube<T>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
{
    fn from(tube_like: T) -> Self {
        Self {
            inner: tube_like,
            timeout: Duration::MAX,
            read_buf_logged: 0,
        }
    }
}
