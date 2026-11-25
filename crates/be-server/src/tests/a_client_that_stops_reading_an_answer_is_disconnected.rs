use tokio::{io::Interest, net::TcpSocket};

use super::{
    an_object_uploads_and_downloads_with_presigned_s3_urls::{EXPIRES, endpoint, upload},
    *,
};
use crate::{s3::presign_get, store::MAX_OBJECT_BYTES};

/// A client that asks for a large object and reads none of it holds its connection only until
/// the answer has gone `write_timeout` without a byte taken: then the server resets it.
#[tokio::test]
async fn a_client_that_stops_reading_an_answer_is_disconnected() {
    let harness = Harness::configured(ServerConfig {
        write_timeout: Duration::from_millis(200),
        ..ServerConfig::OPEN
    })
    .await;
    let (bucket, credentials) = endpoint(&harness);
    // More than the server's socket and the client's buffer together.
    let bytes = vec![7u8; MAX_OBJECT_BYTES];
    assert_eq!(upload(&bucket, &credentials, &bytes).await.status, 200);
    let url = presign_get(&bucket, &credentials, Hash::of(&bytes), EXPIRES);

    let socket = TcpSocket::new_v4().unwrap();
    socket.set_recv_buffer_size(4096).unwrap();
    let mut stream = socket.connect(harness.server.address).await.unwrap();
    let request = format!(
        "GET {}?{} HTTP/1.1\r\nHost: {}\r\n\r\n",
        url.path(),
        url.query().unwrap_or_default(),
        harness.server.address,
    );
    stream.write_all(request.as_bytes()).await.unwrap();

    // Reading nothing, the client hears of the reset as its socket's error. Without the
    // deadline the server would go on waiting for it to read for as long as it stays.
    let ready = tokio::time::timeout(Duration::from_secs(30), stream.ready(Interest::ERROR))
        .await
        .expect("the server gives up on a client that takes none of its answer")
        .unwrap();
    assert!(ready.is_error());
    let mut read = Vec::new();
    let ended = stream.read_to_end(&mut read).await;
    assert!(
        ended.is_err() || read.len() < bytes.len(),
        "the answer was cut off, not finished"
    );
}
