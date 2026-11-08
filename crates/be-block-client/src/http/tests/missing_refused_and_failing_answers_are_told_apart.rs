use super::*;

#[tokio::test]
async fn missing_refused_and_failing_answers_are_told_apart() {
    let (url, served) =
        serve_once("HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNoSuchKey").await;
    assert_eq!(get(&url, 1024).await, Err(Error::NotFound));
    served.await.unwrap();

    let (url, served) =
        serve_once("HTTP/1.1 403 Forbidden\r\nContent-Length: 21\r\n\r\nSignatureDoesNotMatch")
            .await;
    assert_eq!(
        put(&url, &[], b"bytes".to_vec()).await,
        Err(Error::Refused {
            status: 403,
            message: "SignatureDoesNotMatch".to_owned()
        })
    );
    served.await.unwrap();

    // A refusal too long to read whole is still a refusal.
    let mut long = b"HTTP/1.1 400 Bad Request\r\nContent-Length: 100000\r\n\r\n".to_vec();
    long.resize(long.len() + 100_000, b'x');
    let (url, served) = serve_once(long).await;
    assert_eq!(
        get(&url, 1024).await,
        Err(Error::Refused {
            status: 400,
            message: String::new()
        })
    );
    served.await.unwrap();

    let (url, served) =
        serve_once("HTTP/1.1 503 Service Unavailable\r\nContent-Length: 4\r\n\r\nbusy").await;
    assert!(matches!(get(&url, 1024).await, Err(Error::Failed(_))));
    served.await.unwrap();

    let (url, served) = serve_once("not http at all\r\n\r\n").await;
    assert!(matches!(get(&url, 1024).await, Err(Error::Failed(_))));
    served.await.unwrap();
}
