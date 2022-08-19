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
//! async fn main() -> io::Result<()> {
//!     let mut p = Tube::process("/usr/bin/cat")?;
//!     p.send(b"Hello World!").await?;
//!     let output = p.recv_until(b"World").await?;
//!     assert_eq!(output, b"Hello World");
//!     Ok(())
//! }
//! ```
//!
//! Any type that implement [`AsyncRead`](tokio::io::AsyncRead) + [`AsyncWrite`](tokio::io::AsyncWrite) can
//! make use of [`Tube::new`](crate::tubes::Tube::new) to generate extra methods.
//!
//! ```rust, no_run
//! use io_tubes::tubes::Tube;
//! use std::io;
//! use tokio::net::TcpStream;
//!
//! #[tokio::main]
//! async fn main() -> io::Result<()> {
//!     // The followings are equivalent `Tube<TcpStream>`.
//!     let mut p = Tube::remote("example.com:1337").await?;
//!     let mut p = Tube::new(TcpStream::connect("example.com:1337").await?);
//!
//!     Ok(())
//! }
//! ```
pub mod tubes;
mod utils;
