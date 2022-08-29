use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use log::debug;
use pretty_hex::PrettyHex;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, ReadBuf};

/// A tube-like struct which logs all data passed through it by acting like `tee`.
/// When shutdown is called on this struct, the logger passed to it will not be shutdown.
/// If you wish to shutdown those tubes, you can pass in a mutable reference and perform shutdown
/// manually after the debug tube is shutdown (which ensures the data is flushed to the loggers).
pub struct DebugTube<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    inner: T,
    read_buf_logged: usize,
}

impl<T> DebugTube<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    /// Create a new DebugTube with the supplied logger with initial capacity 8KB
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            read_buf_logged: 0,
        }
    }
}

impl<T> AsyncRead for DebugTube<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        let olen = buf.filled().len();

        if Pin::new(&mut self.inner).poll_read(cx, buf)?.is_pending() {
            return Poll::Pending;
        }

        debug!(target: "Tube::recv", "Received {:?}", buf.filled()[olen..].hex_dump());

        Poll::Ready(Ok(()))
    }
}

// Vectored write is not implemented even if both logger and inner is optimied for vectored write.
// This is due to the need for buffering will cause the slices to be stored in a Vec which defies
// the purpose of a vectored write.
impl<T> AsyncWrite for DebugTube<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
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

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
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

impl<T> AsyncBufRead for DebugTube<T>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<&[u8]>> {
        let Self {
            inner,
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
