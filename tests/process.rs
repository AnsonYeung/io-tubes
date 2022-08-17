use io_tubes::tubes::Tube;
use std::io;

#[tokio::test]
async fn can_create_process() -> io::Result<()> {
    let mut p = Tube::process("/usr/bin/cat")?;
    p.send(b"abcdHi!").await?;
    let result = p.recv_until(b"Hi").await?;
    assert_eq!(result, b"abcdHi");
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
