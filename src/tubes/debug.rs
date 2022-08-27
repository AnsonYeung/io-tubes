use std::{
    cmp::min,
    io,
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, ReadBuf};

pub struct DebugTube<T, U>
where
    T: AsyncRead + AsyncWrite + Unpin,
    U: AsyncWrite + Unpin,
{
    inner: T,
    logger: U,
    read_buf: Vec<u8>,
    read_buf_logged: usize,
    write_buf: Vec<u8>,
}

impl<T, U> DebugTube<T, U>
where
    T: AsyncRead + AsyncWrite + Unpin,
    U: AsyncWrite + Unpin,
{
    /// Create a new DebugTube with the supplied logger with capacity 8KB
    pub fn new(inner: T, logger: U) -> Self {
        Self::with_capacity(8 * 1024, inner, logger)
    }

    /// Create a new DebugTube with the specified capacity
    pub fn with_capacity(capacity: usize, inner: T, logger: U) -> Self {
        Self {
            inner,
            logger,
            read_buf: Vec::with_capacity(capacity),
            read_buf_logged: 0,
            write_buf: Vec::with_capacity(capacity),
        }
    }
}

impl<T, U> AsyncRead for DebugTube<T, U>
where
    T: AsyncRead + AsyncWrite + Unpin,
    U: AsyncWrite + Unpin,
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
impl<T, U> AsyncWrite for DebugTube<T, U>
where
    T: AsyncRead + AsyncWrite + Unpin,
    U: AsyncWrite + Unpin,
{
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        let Self {
            inner,
            logger,
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
            let result = Pin::new(&mut *logger).poll_write(cx, write_buf)?;
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
            logger,
            read_buf: _,
            read_buf_logged: _,
            write_buf,
        } = self.get_mut();

        if !write_buf.is_empty() {
            loop {
                let result = Pin::new(&mut *logger).poll_write(cx, write_buf)?;
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
            if Pin::new(logger).poll_flush(cx)?.is_pending() {
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

impl<T, U> AsyncBufRead for DebugTube<T, U>
where
    T: AsyncRead + AsyncWrite + Unpin,
    U: AsyncWrite + Unpin,
{
    fn poll_fill_buf(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<&[u8]>> {
        // while using unsafe code can lead to better performance, it's also easlier to make bugs.
        if self.read_buf_logged > 0 {
            let read_buf_logged = self.read_buf_logged;
            return Poll::Ready(Ok(&self.get_mut().read_buf[..read_buf_logged]));
        }

        let Self {
            inner,
            logger,
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
            let result = Pin::new(&mut *logger).poll_write(cx, &read_buf[*read_buf_logged..len])?;
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

    fn consume(self: Pin<&mut Self>, amt: usize) {
        self.get_mut().read_buf.drain(..amt);
    }
}

#[cfg(test)]
mod tests {
    use super::DebugTube;
    use crate::tubes::{ProcessTube, Tube};
    use std::io;

    #[tokio::test]
    async fn can_debug_tube() -> io::Result<()> {
        let mut logger = Vec::new();
        let p = ProcessTube::new("/usr/bin/cat")?;
        let mut p = Tube::new(DebugTube::new(p, &mut logger));
        p.send_line("abc").await?;
        assert_eq!(p.recv_line().await?, b"abc\n");
        assert_eq!(logger, b"abc\nabc\n");
        Ok(())
    }
}
