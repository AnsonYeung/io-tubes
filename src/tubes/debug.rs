use std::{
    cmp::min,
    io,
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, ReadBuf};

/// A tube-like struct which logs all data passed through it by acting like `tee`.
/// When shutdown is called on this struct, the logger passed to it will not be shutdown.
/// If you wish to shutdown those tubes, you can pass in a mutable reference and perform shutdown
/// manually after the debug tube is shutdown (which ensures the data is flushed to the loggers).
pub struct DebugTube<T, U, V>
where
    T: AsyncRead + AsyncWrite + Unpin,
    U: AsyncWrite + Unpin,
    V: AsyncWrite + Unpin,
{
    inner: T,
    pub(super) read_logger: U,
    pub(super) write_logger: V,
    read_buf: Vec<u8>,
    read_buf_logged: usize,
    write_buf: Vec<u8>,
}

impl<T, U, V> DebugTube<T, U, V>
where
    T: AsyncRead + AsyncWrite + Unpin,
    U: AsyncWrite + Unpin,
    V: AsyncWrite + Unpin,
{
    /// Create a new DebugTube with the supplied logger with capacity 8KB
    pub fn new(inner: T, read_logger: U, write_logger: V) -> Self {
        Self::with_capacity(8 * 1024, inner, read_logger, write_logger)
    }

    /// Create a new DebugTube with the specified capacity
    pub fn with_capacity(capacity: usize, inner: T, read_logger: U, write_logger: V) -> Self {
        Self {
            inner,
            read_logger,
            write_logger,
            read_buf: Vec::with_capacity(capacity),
            read_buf_logged: 0,
            write_buf: Vec::with_capacity(capacity), // this should auto grow, using capacity
                                                     // provided as a heuristic here.
        }
    }
}

impl<T, U, V> AsyncRead for DebugTube<T, U, V>
where
    T: AsyncRead + AsyncWrite + Unpin,
    U: AsyncWrite + Unpin,
    V: AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        let read_buf = match self.as_mut().poll_fill_buf(cx)? {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(buf) => buf,
        };
        let remaining = min(buf.remaining(), read_buf.len());
        buf.put_slice(&read_buf[..remaining]);
        self.as_mut().consume(remaining);
        Poll::Ready(Ok(()))
    }
}

// Vectored write is not implemented even if both logger and inner is optimied for vectored write.
// This is due to the need for buffering will cause the slices to be stored in a Vec which defies
// the purpose of a vectored write.
impl<T, U, V> AsyncWrite for DebugTube<T, U, V>
where
    T: AsyncRead + AsyncWrite + Unpin,
    U: AsyncWrite + Unpin,
    V: AsyncWrite + Unpin,
{
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        let Self {
            inner,
            read_logger: _,
            write_logger,
            read_buf: _,
            read_buf_logged: _,
            write_buf,
        } = self.get_mut();
        let mut ready = false;
        let mut numb = 0;

        // invoke underlying write
        loop {
            let result = Pin::new(&mut *inner).poll_write(cx, &buf[numb..])?;
            if let Poll::Ready(_numb) = result {
                ready = true;
                numb += _numb;
                if numb == buf.len() || _numb == 0 {
                    break;
                }
            } else {
                break;
            }
        }

        // write to logger
        write_buf.extend(&buf[..numb]);
        loop {
            let result = Pin::new(&mut *write_logger).poll_write(cx, write_buf)?;
            if let Poll::Ready(numb) = result {
                if numb == 0 {
                    break;
                }
                write_buf.drain(..numb);
                if write_buf.is_empty() {
                    break;
                }
            } else {
                break;
            }
        }

        if ready {
            Poll::Ready(Ok(numb))
        } else {
            Poll::Pending
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        let mut ready = true;

        let Self {
            inner,
            read_logger: _,
            write_logger,
            read_buf: _,
            read_buf_logged: _,
            write_buf,
        } = self.get_mut();

        if !write_buf.is_empty() {
            loop {
                let result = Pin::new(&mut *write_logger).poll_write(cx, write_buf)?;
                if let Poll::Ready(numb) = result {
                    ready = true;
                    if numb == 0 {
                        break;
                    }
                    write_buf.drain(..numb);
                    if write_buf.is_empty() {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        if write_buf.is_empty() {
            if Pin::new(write_logger).poll_flush(cx)?.is_pending() {
                ready = false;
            }
        } else {
            ready = false;
        }

        if Pin::new(inner).poll_flush(cx)?.is_pending() {
            ready = false;
        }

        if ready {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        // Do not perform shutdown on the logger, since the user can call shutdown if they passed a
        // mutable reference to the constructor.
        let mut ready = true;

        // flush the logger
        if self.as_mut().poll_flush(cx)?.is_pending() {
            ready = false;
        }

        // shutdown inner
        if Pin::new(&mut self.inner).poll_shutdown(cx)?.is_pending() {
            ready = false;
        }

        if ready {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

impl<T, U, V> AsyncBufRead for DebugTube<T, U, V>
where
    T: AsyncRead + AsyncWrite + Unpin,
    U: AsyncWrite + Unpin,
    V: AsyncWrite + Unpin,
{
    fn poll_fill_buf(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<&[u8]>> {
        // while using unsafe code can lead to better performance, it's also easlier to make bugs.
        if self.read_buf_logged > 0 {
            let read_buf_logged = self.read_buf_logged;
            return Poll::Ready(Ok(&self.get_mut().read_buf[..read_buf_logged]));
        }

        let Self {
            inner,
            read_logger,
            write_logger: _,
            read_buf,
            read_buf_logged,
            write_buf: _,
        } = self.deref_mut();

        let mut len = read_buf.len();
        read_buf.resize(read_buf.capacity(), 0);
        let mut ready = true;

        while ready {
            ready = false;

            // invoke underlying read
            let mut buf = ReadBuf::new(&mut read_buf[len..]);
            let result = Pin::new(&mut *inner).poll_read(cx, &mut buf);
            if result?.is_ready() {
                ready = true;
                len += buf.filled().len();
            }

            // write to logger
            let result =
                Pin::new(&mut *read_logger).poll_write(cx, &read_buf[*read_buf_logged..len])?;
            if let Poll::Ready(numb) = result {
                if numb == 0 {
                    read_buf.truncate(len);
                    if ready || *read_buf_logged > 0 {
                        // both logger and inner are ready, and we can't move forward because we
                        // can't write to logger.
                        let read_buf_logged = *read_buf_logged;
                        return Poll::Ready(Ok(&self.get_mut().read_buf[..read_buf_logged]));
                    }
                    // inner is pending, there can be data incoming so we'll continue after inner
                    // is ready.
                    return Poll::Pending;
                }
                *read_buf_logged += numb;
                ready = true;
            }
        }

        read_buf.truncate(len);

        if *read_buf_logged > 0 {
            let read_buf_logged = *read_buf_logged;
            return Poll::Ready(Ok(&self.get_mut().read_buf[..read_buf_logged]));
        }

        Poll::Pending
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        self.read_buf_logged -= amt;
        self.get_mut().read_buf.drain(..amt);
    }
}

#[cfg(test)]
mod tests {
    use tokio::{io::AsyncWriteExt, net::TcpStream};

    use super::DebugTube;
    use crate::tubes::{Listener, ProcessTube, Tube};
    use std::{
        io,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    #[tokio::test]
    async fn debug_tube_ok() -> io::Result<()> {
        let mut read_logger = Vec::new();
        let mut write_logger = Vec::new();
        let p = ProcessTube::new("/usr/bin/cat")?;
        let mut p = Tube::new(DebugTube::new(p, &mut read_logger, &mut write_logger));
        p.send_line("abc").await?;
        assert_eq!(p.recv_line().await?, b"abc\n");
        p.send_line("def").await?;
        assert_eq!(p.recv_line().await?, b"def\n");
        assert_eq!(read_logger, b"abc\ndef\n");
        assert_eq!(write_logger, b"abc\ndef\n");
        Ok(())
    }

    #[tokio::test]
    async fn debug_tube_ok_tcp() -> io::Result<()> {
        let mut read_logger = Vec::new();
        let mut write_logger = Vec::new();
        let l = Listener::listen().await?;
        let p =
            TcpStream::connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), l.port()?)).await?;
        let mut p = Tube::new(DebugTube::new(p, &mut read_logger, &mut write_logger));
        p.send_line("abc").await?;
        p.send_line("def").await?;
        p.send_line("Client Hello").await?;
        p.flush().await?;
        let mut s = l.accept().await?;
        s.send_line("Server Hello").await?;
        s.flush().await?;
        assert_eq!(p.recv_line().await?, b"Server Hello\n");
        assert_eq!(read_logger, b"Server Hello\n");
        assert_eq!(write_logger, b"abc\ndef\nClient Hello\n");
        Ok(())
    }
}
