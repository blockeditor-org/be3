use super::*;

async fn settle() {
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(start_paused = true)]
async fn a_slow_post_or_answer_goes_on_while_its_bytes_keep_moving() {
    let large = vec![7u8; be_block_protocol::MAX_FRAME_BYTES];

    // An upload the server takes slowly.
    let moved = Moved::new();
    let (client, mut server) = tokio::io::duplex(64 * 1024);
    let mut stream: Box<dyn Io> = Box::new(Moving {
        inner: Box::new(client),
        moved: moved.clone(),
    });
    let posting = tokio::spawn({
        let (moved, large) = (moved.clone(), large.clone());
        async move {
            let posted = while_moving(
                async {
                    stream
                        .write_all(&large)
                        .await
                        .map_err(|error| error.to_string())
                },
                &moved,
            )
            .await;
            (posted, stream)
        }
    });
    let started = tokio::time::Instant::now();
    let mut taken = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    while taken.len() < large.len() {
        let read = server.read(&mut chunk).await.unwrap();
        taken.extend_from_slice(&chunk[..read]);
        if taken.len() % (large.len() / 4) < read {
            settle().await;
            tokio::time::advance(QUIET_WAIT / 2).await;
        }
    }
    let (posted, mut stream) = posting.await.unwrap();
    assert!(
        started.elapsed() > QUIET_WAIT,
        "the upload outlasted the wait"
    );
    assert_eq!(
        posted,
        Ok(()),
        "an upload whose bytes kept moving went through"
    );
    assert!(taken == large);

    // An answer that arrives slowly.
    let mut answer =
        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", large.len()).into_bytes();
    answer.extend_from_slice(&large);
    let reading = tokio::spawn({
        let moved = moved.clone();
        async move {
            let read = while_moving(
                async {
                    read_response(&mut stream)
                        .await
                        .unwrap_or_else(|Stale| Err("the connection went stale".to_owned()))
                },
                &moved,
            )
            .await;
            (read, stream)
        }
    });
    let started = tokio::time::Instant::now();
    for part in answer.chunks(answer.len() / 4 + 1) {
        server.write_all(part).await.unwrap();
        settle().await;
        tokio::time::advance(QUIET_WAIT / 2).await;
    }
    let (read, mut stream) = reading.await.unwrap();
    assert!(
        started.elapsed() > QUIET_WAIT,
        "the answer outlasted the wait"
    );
    assert!(
        matches!(&read, Ok(Response { status: 200, body, .. }) if *body == large),
        "an answer whose bytes kept moving was read whole"
    );

    // Silence.
    let silent = tokio::time::Instant::now();
    let read = while_moving(
        async {
            read_response(&mut stream)
                .await
                .unwrap_or_else(|Stale| Err("the connection went stale".to_owned()))
        },
        &moved,
    )
    .await;
    assert!(read.is_err(), "a server that sends nothing is given up on");
    assert!(silent.elapsed() >= QUIET_WAIT);
    drop(server);
}
