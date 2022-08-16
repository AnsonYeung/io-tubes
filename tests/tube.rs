use io_tubes::{traits::*, tubes::Tube};
use std::io;

#[tokio::test]
async fn can_recv_until() -> io::Result<()> {
    let mut fake_reader: &[u8] = b"The quick brown fox jumps over the lazy dog";

    // can recv_until
    assert_eq!(
        fake_reader.recv_until(b"fox").await?,
        b"The quick brown fox"
    );

    // can recv more
    assert_eq!(fake_reader.recv_until(b"over").await?, b" jumps over");

    // can recv until EOF
    assert_eq!(fake_reader.recv_until(b"\0").await?, b" the lazy dog");

    Ok(())
}

#[tokio::test]
async fn can_send_line_after() -> io::Result<()> {
    let mut p = Tube::process("/usr/bin/cat")?;

    p.send(b"Hello, what's your name? ").await?;
    assert_eq!(
        p.send_line_after(b"name", b"test").await?,
        b"Hello, what's your name"
    );
    assert_eq!(p.recv_line().await?, b"? test\n");

    Ok(())
}
