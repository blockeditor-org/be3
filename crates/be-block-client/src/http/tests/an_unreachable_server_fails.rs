use super::*;

#[tokio::test]
async fn an_unreachable_server_fails() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    assert!(matches!(
        get(&format!("http://{address}/object"), 1024).await,
        Err(Error::Failed(_))
    ));
    assert!(matches!(
        get("ftp://example.com/object", 1024).await,
        Err(Error::Invalid(_))
    ));
}
