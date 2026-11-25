use super::*;

/// hyper writes an answer for as long as the client takes to read it. A client that asks for a
/// large one (an S3 object) and reads none of it is given up once the answer has made no
/// progress for `IDLE_TIMEOUT`, as `http::serve` serves every connection, rather than holding
/// its connection and place until it goes.
#[tokio::test(start_paused = true)]
async fn a_client_that_stops_reading_an_answer_is_given_up() {
    let (mut client, server) = tokio::io::duplex(64 << 10);
    let service = hyper::service::service_fn(|_| async {
        Ok::<_, std::convert::Infallible>(::http::Response::new(axum::body::Body::from(
            vec![7u8; 16 << 20],
        )))
    });
    let serving = tokio::spawn(
        hyper::server::conn::http1::Builder::new()
            .timer(hyper_util::rt::TokioTimer::new())
            .serve_connection(
                hyper_util::rt::TokioIo::new(crate::http::stalled::WriteDeadline::new(
                    server,
                    IDLE_TIMEOUT,
                )),
                service,
            ),
    );
    client
        .write_all(b"GET /storage/objects/large HTTP/1.1\r\nHost: server\r\n\r\n")
        .await
        .unwrap();
    let started = tokio::time::Instant::now();

    let served = tokio::time::timeout(IDLE_TIMEOUT * 2, serving)
        .await
        .expect("the connection ends")
        .unwrap();
    assert!(served.is_err(), "the connection is given up: {served:?}");
    assert_eq!(
        started.elapsed(),
        IDLE_TIMEOUT,
        "once the answer made no progress for that long"
    );
    drop(client);
}
