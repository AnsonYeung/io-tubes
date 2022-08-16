# io-tubes

[![Crates.io](https://img.shields.io/crates/v/io-tubes)](https://crates.io/crates/io-tubes)
[![docs.rs](https://img.shields.io/docsrs/io-tubes)](https://docs.rs/io-tubes)

Provides tube functionality like the python library [pwntools](https://github.com/Gallopsled/pwntools).

Documentation can be found at [docs.rs](https://docs.rs/io-tubes)

## Example
```rust
use io_tubes::traits::*;
use io_tubes::tubes::Tube;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut p = Tube::process("/usr/bin/cat")?;
    p.send(b"Hello World!").await?;
    let output = p.recv_until(b"World").await?;
    assert_eq!(output, b"Hello World");
    p.interactive().await
}
```
