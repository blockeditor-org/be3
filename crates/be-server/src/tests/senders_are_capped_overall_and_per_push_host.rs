use crate::push::Senders;

#[tokio::test]
async fn senders_are_capped_overall_and_per_push_host() {
    let senders = Senders::new(3, 2);

    let first = senders.acquire("a.example").await.unwrap();
    let second = senders.acquire("a.example").await.unwrap();
    // A third to the same host waits for a place, though the server has one free.
    let mut third = Box::pin(senders.acquire("a.example"));
    assert!(futures_lite_poll(&mut third).is_none());
    // Another host still gets the free place; then the server is full for every host.
    let other = senders.acquire("b.example").await.unwrap();
    let mut fourth = Box::pin(senders.acquire("c.example"));
    assert!(futures_lite_poll(&mut fourth).is_none());

    drop(first);
    // The host's place is free, but the server's are all taken: still waiting.
    assert!(futures_lite_poll(&mut third).is_none());
    drop(other);
    assert!(third.await.is_some());
    assert!(fourth.await.is_some());
    drop(second);
}

/// Polls once, with no waker to wake: `None` while the future waits.
fn futures_lite_poll<F: std::future::Future>(future: &mut std::pin::Pin<Box<F>>) -> Option<F::Output> {
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(output) => Some(output),
        std::task::Poll::Pending => None,
    }
}
