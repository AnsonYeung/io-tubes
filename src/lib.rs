//! Tubes
//!
//! Provides tube functionality like the python library [pwntools](https://github.com/Gallopsled/pwntools).
//! The methods will be implemented for structs that implements [`AsyncBufRead`](tokio::io::AsyncBufRead) +
//! [`AsyncWrite`](tokio::io::AsyncWrite).
//! The [`tokio`] library provides types that implements [`AsyncRead`](tokio::io::AsyncRead) +
//! [`AsyncWrite`](tokio::io::AsyncWrite).
//! Using [`BufReader`](tokio::io::BufReader) will implement [`AsyncBufRead`](tokio::io::AsyncBufRead) for types that don't have the
//! trait.
//! Some methods like [`recv_until`](crate::traits::TubeBufRead::recv_until) requires the buffer to store
//! data that remains after the method call.
//!
//! ```rust
//! use io_tubes::traits::*;
//! use io_tubes::tubes::ProcessTube;
//! use tokio::io::BufReader;
//! use std::io;
//!
//! #[tokio::main]
//! async fn main() -> io::Result<()> {
//!     let mut p = BufReader::new(ProcessTube::new("/usr/bin/cat")?);
//!     p.send(b"Hello World!").await?;
//!     let output = p.recv_until(b"World").await?;
//!     assert_eq!(output, b"Hello World");
//!     Ok(())
//! }
//! ```
pub mod traits;
pub mod tubes;
mod utils;
