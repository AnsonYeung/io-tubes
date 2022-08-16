use crate::utils::RecvUntil;
use std::io;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use async_trait::async_trait;

const NEW_LINE: u8 = 0xA;

#[async_trait]
pub trait TubeLike: TubeBufRead + TubeWrite {
    /// ```rust, ignore
    /// async fn send_line_after(&mut self, pattern: &[u8], data: &[u8]) -> io::Result<Vec<u8>>
    /// ```
    /// Send line after receiving the pattern from read.
    async fn send_line_after(&mut self, pattern: &[u8], data: &[u8]) -> io::Result<Vec<u8>> {
        let result = self.recv_until(pattern).await?;
        self.send_line(data).await?;
        Ok(result)
    }
}

impl<T: TubeBufRead + TubeWrite> TubeLike for T {}

#[async_trait]
pub trait TubeRead: AsyncReadExt + Unpin {
    /// ```rust, ignore
    /// async fn recv(&mut self, len: usize) -> io::Result<Vec<u8>>
    /// ```
    /// Receive up to `len` bytes
    async fn recv(&mut self, len: usize) -> io::Result<Vec<u8>> {
        let mut buf = vec![0; len];
        let len = self.read(&mut buf[..]).await?;
        buf.truncate(len);
        Ok(buf)
    }
}

impl<T: AsyncReadExt + Unpin + ?Sized> TubeRead for T {}

#[async_trait]
pub trait TubeBufRead: TubeRead + AsyncBufReadExt + Unpin {
    /// ```rust, ignore
    /// async fn recv_line(&mut self) -> io::Result<Vec<u8>>
    /// ```
    /// Receive until new line (0xA byte) is reached or EOF is reached
    async fn recv_line(&mut self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.read_until(NEW_LINE, &mut buf).await?;
        Ok(buf)
    }

    /// ```rust,ignore
    /// async fn recv_until(&mut self, delims: &[u8]) -> io::Result<Vec<u8>>
    /// ```
    /// Receive until the delims are found or EOF is reached.
    /// A lookup table will be built to enable efficient matching of long patterns.
    fn recv_until(&mut self, delims: &[u8]) -> RecvUntil<Self> {
        RecvUntil::new(self, delims)
    }
}

impl<T: AsyncBufReadExt + Unpin + ?Sized> TubeBufRead for T {}

#[async_trait]
pub trait TubeWrite: AsyncWriteExt + Unpin {
    /// ```rust, ignore
    /// async fn send(&mut self, data: &[u8]) -> io::Result<()>
    /// ```
    /// Send data and flush.
    async fn send(&mut self, data: &[u8]) -> io::Result<()> {
        self.write_all(data).await?;
        self.flush().await
    }

    /// ```rust, ignore
    /// async fn send_line(&mut self, data: &[u8]) -> io::Result<()>
    /// ```
    /// Same as send, but add new line (0xA byte).
    async fn send_line(&mut self, data: &[u8]) -> io::Result<()> {
        self.write_all(data).await?;
        self.write_all(&[NEW_LINE]).await?;
        self.flush().await
    }
}

impl<T: AsyncWriteExt + Unpin> TubeWrite for T {}
