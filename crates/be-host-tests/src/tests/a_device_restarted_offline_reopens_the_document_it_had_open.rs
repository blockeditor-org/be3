use super::*;

/// A device restarted offline opens from what it keeps: My files and the document it had open,
/// which takes edits.
#[test]
fn a_device_restarted_offline_reopens_the_document_it_had_open() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut phone = HostTest::device(&dev);
    add_to_my_files(&mut phone, "Text");
    phone.click("#text.surface");
    phone.type_text("Recipes");
    phone.synced();

    phone.offline();
    phone.restart();
    phone.wait("TreeItem \"Recipes\"");
    phone.click("#text.surface");
    phone.key("ctrl+End");
    phone.type_text(" for soup");
    phone.wait("Recipes for soup");
    phone.close();
}
