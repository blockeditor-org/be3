use super::*;

/// When the network starts dropping what the device sends, before the device notices, its
/// editors and files keep working from what it keeps: nothing waits on a send that cannot end.
#[test]
#[ignore = "sending sealed edits, listing revisions and indexing count as busy while the server silently stops answering"]
fn typing_as_the_connection_silently_drops_leaves_the_host_settled() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut phone = HostTest::device(&dev);
    add_to_my_files(&mut phone, "Text");
    phone.click("#text.surface");
    phone.type_text("Recipes");
    phone.synced();

    phone.unreachable();
    phone.type_text(" for soup");
    phone.run(&["settle"]);
    phone.pause(10_000);
    phone.run(&["settle"]);
    phone.wait("Recipes for soup");
    phone.online();
    phone.synced();
    phone.close();
}
