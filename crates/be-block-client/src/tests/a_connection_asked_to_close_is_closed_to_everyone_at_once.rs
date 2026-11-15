use super::*;

/// A connection asked to close reads as closed at once, before its worker has shut the socket:
/// whoever looks for a live connection to borrow next (be-host's link, after a stack stops)
/// never takes one that is about to drop.
#[tokio::test]
async fn a_connection_asked_to_close_is_closed_to_everyone_at_once() {
    let harness = Harness::start().await;
    let connection = Connection::connect(&harness.url, &Clock::real())
        .await
        .expect("the connection opens");
    assert!(!connection.is_closed());

    connection.close();
    assert!(
        connection.is_closed(),
        "a connection asked to close is never handed on"
    );
    let mut closed = connection.closed();
    closed
        .wait_for(|closed| *closed)
        .await
        .expect("the socket shuts");
    harness.stop().await;
}
