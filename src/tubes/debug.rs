use std::{
    collections::VecDeque,
    io,
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
            if let Poll::Ready(result) = result {
                ready = true;
                if let Err(e) = result {
                    return Poll::Ready(Err(e));
                }
                let new = &buf.filled()[prev_len..];
                read_buf.extend(new);
                prev_len = buf.filled().len();
            } else {
                break;
            }
        }

        // write to logger
        loop {
            let result = Pin::new(&mut *logger).poll_write(cx, read_buf.as_slices().0);
            if let Poll::Ready(result) = result {
                match result {
                    Err(e) => return Poll::Ready(Err(e)),
                    Ok(numb) => {
                        if numb == 0 {
                            break;
                        }
                        read_buf.drain(..numb);
                        if read_buf.is_empty() {
                            break;
                        }
                    }
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
            let result = Pin::new(&mut *inner).poll_write(cx, &buf[numb..]);
            if let Poll::Ready(result) = result {
                ready = true;
                match result {
                    Err(e) => return Poll::Ready(Err(e)),
                    Ok(_numb) => {
                        numb += _numb;
                        if numb == buf.len() || _numb == 0 {
                            break;
                        }
                    }
                }
            } else {
                break;
            }
        }

        // write to logger
        write_buf.extend(&buf[..numb]);
        loop {
            let result = Pin::new(&mut *logger).poll_write(cx, write_buf.as_slices().0);
            if let Poll::Ready(result) = result {
                match result {
                    Err(e) => return Poll::Ready(Err(e)),
                    Ok(numb) => {
                        if numb == 0 {
                            break;
                        }
                        write_buf.drain(..numb);
                        if write_buf.is_empty() {
                            break;
                        }
                    }
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
                let result = Pin::new(&mut *logger).poll_write(cx, write_buf.as_slices().0);
                if let Poll::Ready(result) = result {
                    ready = true;
                    match result {
                        Err(e) => return Poll::Ready(Err(e)),
                        Ok(numb) => {
                            if numb == 0 {
                                break;
                            }
                            write_buf.drain(..numb);
                            if write_buf.is_empty() {
                                break;
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }

        if write_buf.is_empty() {
            if let Poll::Ready(result) = Pin::new(logger).poll_flush(cx) {
                if let Err(e) = result {
                    return Poll::Ready(Err(e));
                }
            } else {
                ready = false;
            }
        } else {
            ready = false;
        }

        if let Poll::Ready(result) = Pin::new(inner).poll_flush(cx) {
            if let Err(e) = result {
                return Poll::Ready(Err(e));
            }
        } else {
            ready = false;
        }

        if ready {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        // Ideally, the shutdown of the logger should be a no-op
        // Since we own the logger here, we'll still propagate shutdown to it.
        let mut ready = true;
        if let Poll::Ready(result) = self.as_mut().poll_flush(cx) {
            if let Err(e) = result {
                return Poll::Ready(Err(e));
            }
            if let Poll::Ready(result) = Pin::new(&mut self.logger).poll_shutdown(cx) {
                if let Err(e) = result {
                    return Poll::Ready(Err(e));
                }
            } else {
                ready = false;
            }
        } else {
            ready = false;
        }

        if let Poll::Ready(result) = Pin::new(&mut self.inner).poll_shutdown(cx) {
            if let Err(e) = result {
                return Poll::Ready(Err(e));
            }
        } else {
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
mod test {
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
