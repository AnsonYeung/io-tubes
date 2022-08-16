//! Tubes
//!
//! Provides tube functionality like the python library [pwntools](https://github.com/Gallopsled/pwntools).
//!
//! The tube methods will be implemented as a trait for all structs that implements
//! [`AsyncBufRead`](tokio::io::AsyncBufRead) + [`AsyncWrite`](tokio::io::AsyncWrite).
//! For tubes not provided by this library, [`tokio`] library can provide other types that implements
//! [`AsyncRead`](tokio::io::AsyncRead) + [`AsyncWrite`](tokio::io::AsyncWrite).
//! Using [`Tube::new`](io_tubes::tubes::Tube::new) will use [`BufReader`](tokio::io::BufReader) to implement
//! [`AsyncBufRead`](tokio::io::AsyncBufRead) and add extra functionality (WIP) to the tube.
//!
//! ```rust
//! use io_tubes::traits::*;
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
pub mod traits;
pub mod tubes;
mod utils;
