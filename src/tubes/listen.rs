use std::{io, net::SocketAddr};

use tokio::{
    io::BufReader,
    net::{TcpListener, TcpStream, ToSocketAddrs},
};

use super::Tube;

/// A TcpListener that returns Tube when a connection is accepted.
pub struct Listener {
    pub inner: TcpListener,
}

impl Listener {
    /// Create a listener by binding to the supplied address.
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        Ok(Listener {
            inner: TcpListener::bind(addr).await?,
        })
    }

    /// Create a listener by binding to 0.0.0.0:0
    pub async fn listen() -> io::Result<Listener> {
        Listener::bind("0.0.0.0:0").await
    }

    /// Accepts a connection.
    pub async fn accept(&self) -> io::Result<Tube<BufReader<TcpStream>>> {
        Ok(Tube::new(self.inner.accept().await?.0))
    }

    /// Returns the port that is listened.
    pub fn port(&self) -> io::Result<u16> {
        Ok(match self.inner.local_addr()? {
            SocketAddr::V4(ip) => ip.port(),
            SocketAddr::V6(ip) => ip.port(),
        })
    }
}


impl From<TcpListener> for Listener {
    fn from(inner: TcpListener) -> Self {
        Self { inner }
    }
}

impl From<Listener> for TcpListener {
    fn from(listener: Listener) -> Self {
        listener.inner
    }
}
