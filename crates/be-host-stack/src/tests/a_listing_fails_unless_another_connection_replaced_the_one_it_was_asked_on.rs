use be_block_client::ClientError;

#[test]
fn a_listing_fails_unless_another_connection_replaced_the_one_it_was_asked_on() {
    let dropped = || ClientError::Disconnected("the connection closed".to_owned());
    let asked_again = crate::worker::asked_again;

    // On the connection that is current, a request that could not be carried is a failure: a
    // slow server is not asked again forever, and the connection's close relists.
    assert!(!asked_again(&dropped(), true, true));
    assert!(!asked_again(&dropped(), true, false));

    // On one another has replaced, while the new one is up, it says nothing of the listing.
    assert!(asked_again(&dropped(), false, true));
    // With nothing up it fails, and the next connection to arrive relists.
    assert!(!asked_again(&dropped(), false, false));

    // What the server answered is a failure whichever connection it came on.
    let refused = ClientError::Unexpected;
    assert!(!asked_again(&refused, false, true));
}
