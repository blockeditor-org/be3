use super::*;

/// When the network starts dropping what the device sends, before the device notices, its
/// editors and files keep working from what it keeps: nothing waits on a send that cannot end.
#[test]
fn typing_as_the_connection_silently_drops_leaves_the_host_settled() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut phone = HostTest::device(&dev);
    add_to_my_files(&mut phone, "Text");
    phone.click("#text.surface");
    phone.type_text("Recipes");
    phone.synced();

    phone.unreachable();
    phone.run(&["--no-settle", "type", " for soup"]);
    // The device notices the silent drop once its ping goes unanswered, about 25 s on; nothing it
    // settles on may wait for that.
    let settled = phone.within(std::time::Duration::from_secs(15), &["settle"]);
    assert!(
        settled.is_ok(),
        "the host settles without waiting on the server: {:?}",
        settled.as_ref().err()
    );
    phone.pause(10_000);
    phone.run(&["settle"]);
    phone.wait("Recipes for soup");
    phone.online();
    phone.synced();
    phone.close();
}
