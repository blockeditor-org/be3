use super::*;

#[tokio::test]
async fn an_answer_longer_than_the_limit_is_refused() {
    let large = vec![7u8; 64 * 1024];

    // A length over the limit is refused before the body is read.
    let mut answer =
        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", large.len()).into_bytes();
    answer.extend_from_slice(&large);
    let (url, served) = serve_once(answer.clone()).await;
    assert_eq!(
        get(&url, large.len() - 1).await,
        Err(Error::TooLarge {
            limit: large.len() - 1
        })
    );
    served.await.unwrap();
    let (url, served) = serve_once(answer).await;
    assert_eq!(get(&url, large.len()).await.unwrap(), large);
    served.await.unwrap();

    // So is a chunked body once its chunks add up past it.
    let mut chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
    for part in large.chunks(1000) {
        chunked.extend_from_slice(format!("{:x}\r\n", part.len()).as_bytes());
        chunked.extend_from_slice(part);
        chunked.extend_from_slice(b"\r\n");
    }
    chunked.extend_from_slice(b"0\r\n\r\n");
    let (url, served) = serve_once(chunked.clone()).await;
    assert_eq!(
        get(&url, 10_500).await,
        Err(Error::TooLarge { limit: 10_500 })
    );
    served.await.unwrap();
    let (url, served) = serve_once(chunked).await;
    assert_eq!(get(&url, large.len()).await.unwrap(), large);
    served.await.unwrap();

    // And a body that runs to the end of the connection.
    let mut unbounded = b"HTTP/1.1 200 OK\r\n\r\n".to_vec();
    unbounded.extend_from_slice(&large);
    let (url, served) = serve_once(unbounded).await;
    assert_eq!(get(&url, 100).await, Err(Error::TooLarge { limit: 100 }));
    served.await.unwrap();
}
