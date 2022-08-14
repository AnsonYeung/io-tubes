# Crate `io-tubes`

Provides tube functionality like the python library [pwntools](https://github.com/Gallopsled/pwntools).

## Example
```rust
use io_tubes::traits::*;
use io_tubes::tubes::ProcessTube;
use tokio::io::BufReader;
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut p = BufReader::new(ProcessTube::new("/usr/bin/cat")?);
    p.send(b"Hello World!").await?;
    let output = p.recv_until(b"World").await?;
    assert_eq!(output, b"Hello World");
    Ok(())
}
```
