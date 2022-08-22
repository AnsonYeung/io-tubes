# io-tubes

[![Crates.io](https://img.shields.io/crates/v/io-tubes)](https://crates.io/crates/io-tubes)
[![docs.rs](https://img.shields.io/docsrs/io-tubes)](https://docs.rs/io-tubes)

Provides tube functionality like the python library [pwntools](https://github.com/Gallopsled/pwntools).

More examples and documentation can be found at [docs.rs](https://docs.rs/io-tubes)

## Example
```rust
use io_tubes::tubes::Tube;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut p = Tube::process("/usr/bin/cat")?;

    // "Hello World!" will be automatically converted to `&[u8]`
    // Alternatively, you can explicitly use b"Hello World!" if it contains invalid UTF-8.
    p.send("Hello World!").await?;

    // You can use any type that implements `AsRef<[u8]>`
    let output = p.recv_until(b"World".to_vec()).await?;
    assert_eq!(output, b"Hello World");
    Ok(())
}
```
