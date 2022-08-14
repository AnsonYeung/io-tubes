use io_tubes::traits::*;

#[tokio::test]
async fn can_recv_until() {
    let mut fake_reader: &[u8] = b"The quick brown fox jumps over the lazy dog";

    // can recv_until
    let result = fake_reader.recv_until(b"fox").await;
    assert!(result.is_ok());
    if let Ok(buf) = result {
        assert_eq!(buf, b"The quick brown fox")
    }

    // can recv more
    let result = fake_reader.recv_until(b"over").await;
    assert!(result.is_ok());
    if let Ok(buf) = result {
        assert_eq!(buf, b" jumps over")
    }

    // can recv until EOF
    let result = fake_reader.recv_until(b"\0").await;
    assert!(result.is_ok());
    if let Ok(buf) = result {
        assert_eq!(buf, b" the lazy dog")
    }
}
