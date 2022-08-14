use std::io;

use io_tubes::traits::*;
use io_tubes::tubes::ProcessTube;
use tokio::io::BufReader;

#[tokio::test]
async fn can_create_process() -> io::Result<()> {
    let p = ProcessTube::new("/usr/bin/cat")?;
    let mut p = BufReader::new(p);
    p.send(b"abcdHi!").await?;
    let result = p.recv_until(b"Hi").await?;
    assert_eq!(result, b"abcdHi");
    Ok(())
}
