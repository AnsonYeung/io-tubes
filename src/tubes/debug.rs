use std::{
    collections::VecDeque,
    io::{self, IoSlice},
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub struct DebugTube<T, U>
where
    T: AsyncRead + AsyncWrite + Unpin,
    U: AsyncWrite + Unpin,
{
    inner: T,
    logger: U,
    read_buf: VecDeque<u8>,
    write_buf: VecDeque<u8>,
}

impl<T, U> DebugTube<T, U>
where
    T: AsyncRead + AsyncWrite + Unpin,
    U: AsyncWrite + Unpin,
{
    /// Create a new DebugTube with the supplied logger
    pub fn new(inner: T, logger: U) -> Self {
        Self {
            inner,
            logger,
            read_buf: VecDeque::new(),
            write_buf: VecDeque::new(),
        }
    }
}

impl<T, U> AsyncRead for DebugTube<T, U>
where
    T: AsyncRead + AsyncWrite + Unpin,
    U: AsyncWrite + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        let Self {
            inner,
            logger,
            read_buf,
            write_buf: _,
        } = self.get_mut();
        let mut prev_len = buf.filled().len();
        let mut ready = false;

        // invoke underlying read
        loop {
            let result = Pin::new(&mut *inner).poll_read(cx, buf);
            if result?.is_ready() {
                ready = true;
                let new = &buf.filled()[prev_len..];
                read_buf.extend(new);
                prev_len = buf.filled().len();
            } else {
                break;
            }
        }

        // write to logger
        loop {
            let result = Pin::new(&mut *logger).poll_write_vectored(
                cx,
                &[
                    IoSlice::new(read_buf.as_slices().0),
                    IoSlice::new(read_buf.as_slices().1),
                ],
            )?;
            if let Poll::Ready(numb) = result {
                if numb == 0 {
                    break;
                }
                read_buf.drain(..numb);
                if read_buf.is_empty() {
                    break;
                }
            } else {
                break;
            }
        }

        if ready {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

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
            let result = Pin::new(&mut *logger).poll_write_vectored(
                cx,
                &[
                    IoSlice::new(write_buf.as_slices().0),
                    IoSlice::new(write_buf.as_slices().1),
                ],
            )?;
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
            write_buf,
        } = self.get_mut();

        if !write_buf.is_empty() {
            loop {
                let result = Pin::new(&mut *logger).poll_write_vectored(
                    cx,
                    &[
                        IoSlice::new(write_buf.as_slices().0),
                        IoSlice::new(write_buf.as_slices().1),
                    ],
                )?;
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

    // TODO: maybe implement vectored write
}

// TODO: implement AsyncBufRead

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
