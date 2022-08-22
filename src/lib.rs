//! Tubes
//!
//! Provides tube functionality like the python library [pwntools](https://github.com/Gallopsled/pwntools).
//!
//! The methods are provided in the struct [`Tube`](tubes::Tube)
//!
//! Example:
//!
//! ```rust
//! use io_tubes::tubes::Tube;
//! use std::io;
//!
//! #[tokio::main]
//! async fn demo() -> io::Result<()> {
//!     let mut p = Tube::process("/usr/bin/cat")?;
//!
//!     // "Hello World!" will be automatically converted to `&[u8]`
//!     // Alternatively, you can explicitly use b"Hello World!" if it contains invalid UTF-8.
//!     p.send("Hello World!").await?;
//!
//!     // You can use any type that implements `AsRef<[u8]>`
//!     let output = p.recv_until(b"World".to_vec()).await?;
//!     assert_eq!(output, b"Hello World");
//!     Ok(())
//! }
//!
//! demo();
//! ```
//!
//! Any type that implement [`AsyncRead`](tokio::io::AsyncRead) + [`AsyncWrite`](tokio::io::AsyncWrite) can
//! make use of [`Tube::new`](crate::tubes::Tube::new) to create a new tube.
//!
//! ```rust
//! use io_tubes::tubes::{Listener, Tube};
//! use std::io;
//! use tokio::net::TcpStream;
//!
//! #[tokio::main]
//! async fn create_remote() -> io::Result<()> {
//!     let l = Listener::bind("0.0.0.0:1337").await?;
//!
//!     // The followings are equivalent `Tube<BufReader<TcpStream>>`.
//!     let mut p = Tube::remote("127.0.0.1:1337").await?;
//!     let mut p = Tube::new(TcpStream::connect("127.0.0.1:1337").await?);
//!
//!     Ok(())
//! }
//!
//! create_remote();
//! ```
pub mod tubes;
mod utils;
