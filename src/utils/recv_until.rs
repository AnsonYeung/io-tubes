use std::{
    future::Future,
    io,
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::AsyncBufRead;

#[must_use = "futures do nothing unless you `.await` or poll them"]
#[derive(Debug)]
pub struct RecvUntil<'a, T: AsyncBufRead + Unpin + ?Sized + 'a> {
    inner: &'a mut T,
    cur_index: usize,
    lookup_table: Vec<[usize; 256]>,
    buf: &'a mut Vec<u8>,
}

impl<'a, T: AsyncBufRead + Unpin + ?Sized + 'a> RecvUntil<'a, T> {
    fn compute_lookup_table(delims: &[u8]) -> Vec<[usize; 256]> {
        let mut lookup_table = Vec::with_capacity(delims.len());
        let mut lps = 0;
        lookup_table.resize(delims.len(), [0; 256]);
        for (row_idx, &delim_last) in delims.iter().enumerate() {
            for new_byte in 0..=255 {
                if new_byte == delim_last {
                    lookup_table[row_idx][new_byte as usize] = row_idx + 1;
                } else {
                    lookup_table[row_idx][new_byte as usize] = lookup_table[lps][new_byte as usize];
                }
            }
            if row_idx != 0 {
                lps = lookup_table[lps][delim_last as usize];
            }
        }
        lookup_table
    }

    pub fn new(inner: &'a mut T, delims: &[u8], buf: &'a mut Vec<u8>) -> Self {
        Self {
            inner,
            cur_index: 0,
            lookup_table: Self::compute_lookup_table(delims),
            buf,
        }
    }
}

impl<'a, T: AsyncBufRead + Unpin + ?Sized + 'a> Future for RecvUntil<'a, T> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        // reborrow everything so borrow checker actually understands
        let Self {
            inner,
            cur_index,
            lookup_table,
            buf,
        } = self.deref_mut();
        let mut inner = Pin::new(inner);
        loop {
            let result = match inner.as_mut().poll_fill_buf(cx) {
                Poll::Ready(result) => result,
                Poll::Pending => return Poll::Pending,
            };
            match result {
                Ok(new_buf) => {
                    for (count, new_byte) in new_buf.iter().enumerate() {
                        *cur_index = lookup_table[*cur_index][*new_byte as usize];
                        if *cur_index == lookup_table.len() {
                            buf.extend_from_slice(&new_buf[..=count]);
                            inner.as_mut().consume(count + 1);
                            return Poll::Ready(Ok(()));
                        }
                    }
                    if new_buf.is_empty() {
                        return Poll::Ready(Ok(()));
                    }
                    buf.extend_from_slice(new_buf);
                    let len = new_buf.len();
                    inner.as_mut().consume(len);
                }
                Err(err) => return Poll::Ready(Err(err)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncBufRead;

    use super::RecvUntil;
    use std::io;

    async fn recv_until<T: AsyncBufRead + Unpin>(
        inner: &mut T,
        delims: &[u8],
    ) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        RecvUntil::new(inner, delims, &mut buf).await?;
        Ok(buf)
    }

    #[tokio::test]
    async fn can_recv_until() -> io::Result<()> {
        let mut fake_reader: &[u8] = b"The quick brown fox jumps over the lazy dog";

        // can recv_until
        assert_eq!(
            recv_until(&mut fake_reader, b"fox").await?,
            b"The quick brown fox"
        );

        // can recv more
        assert_eq!(recv_until(&mut fake_reader, b"over").await?, b" jumps over");

        // can recv until EOF
        assert_eq!(recv_until(&mut fake_reader, b"\0").await?, b" the lazy dog");

        Ok(())
    }
}
