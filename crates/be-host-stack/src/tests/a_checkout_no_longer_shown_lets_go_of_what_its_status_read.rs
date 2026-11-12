use super::*;

#[test]
fn a_checkout_no_longer_shown_lets_go_of_what_its_status_read() {
    let harness = Harness::start();
    harness.connect();
    let versioned = start_versioning("hello\n");
    show_version(versioned.checkout, be_block::CheckoutContent::CONTENT_TYPE);
    wait_until("read the checkout's status and listed its root", |shared| {
        version_status(shared, versioned.checkout).is_some()
            && !shared.graph.wanted(Owner::Version).is_empty()
    });

    want_versions(
        Owner::Editor(versioned.checkout.as_u64_pair().0),
        Vec::new(),
    );
    wait_until("let go of the checkout and its root", |shared| {
        !shared.version_watch.contains(&versioned.checkout)
            && version_status(shared, versioned.checkout).is_none()
            && shared.graph.wanted(Owner::Version).is_empty()
    });
}
