use std::{io, net::SocketAddr};

use tokio::{
    io::BufReader,
    net::{TcpListener, TcpStream, ToSocketAddrs},
};

use super::Tube;

pub struct Listener {
    pub inner: TcpListener,
}

impl Listener {
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        Ok(Listener {
            inner: TcpListener::bind(addr).await?,
        })
    }
    pub async fn accept(&self) -> io::Result<Tube<BufReader<TcpStream>>> {
        Ok(Tube::new(self.inner.accept().await?.0))
    }

    pub fn port(&self) -> io::Result<u16> {
        Ok(match self.inner.local_addr()? {
            SocketAddr::V4(ip) => ip.port(),
            SocketAddr::V6(ip) => ip.port(),
        })
    }
}

pub async fn listen() -> io::Result<Listener> {
    Listener::bind("0.0.0.0:0").await
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
