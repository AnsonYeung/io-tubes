use std::{
    future::Future,
    io::{Error, ErrorKind},
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{self, AsyncBufRead, AsyncWrite, BufReader, Stdin};

use crate::tubes::Tube;

#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub struct Interactive<'a, T>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
{
    inner: &'a mut Tube<T>,
    stdin: BufReader<Stdin>,
}

impl<'a, T> Interactive<'a, T>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
{
    pub fn new(inner: &'a mut Tube<T>) -> Self {
        Self {
            inner,
            stdin: BufReader::new(io::stdin()),
        }
    }
}

impl<'a, T> Future for Interactive<'a, T>
where
    T: AsyncBufRead + AsyncWrite + Unpin,
{
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let Self { inner, stdin } = self.deref_mut();
        let mut stdin = stdin;

        // stdin -> input
        while let Poll::Ready(res) = Pin::new(stdin.deref_mut()).poll_fill_buf(cx) {
            match res {
                Err(e) => return Poll::Ready(Err(e)),
                Ok(buf) => {
                    if buf.is_empty() {
                        return Poll::Ready(Ok(()));
                    }
                    let write_res = Pin::new(inner.deref_mut()).poll_write(cx, buf);
                    if let Poll::Ready(res) = write_res {
                        match res {
                            Err(e) => return Poll::Ready(Err(e)),
                            Ok(amt) => Pin::new(stdin.deref_mut()).consume(amt),
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        // output -> stdout
        while let Poll::Ready(res) = Pin::new(inner.deref_mut()).poll_fill_buf(cx) {
            match res {
                Err(e) => return Poll::Ready(Err(e)),
                Ok(buf) => {
                    if buf.is_empty() {
                        return Poll::Ready(Err(Error::from(ErrorKind::BrokenPipe)));
                    }
                    let write_res = Pin::new(&mut io::stdout()).poll_write(cx, buf);
                    if let Poll::Ready(res) = write_res {
                        match res {
                            Err(e) => return Poll::Ready(Err(e)),
                            Ok(amt) => Pin::new(inner.deref_mut()).consume(amt),
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        Poll::Pending
    }
}
