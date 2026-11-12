use super::*;

/// Offline, an edit is saved on the device and waits to sync: the host says so, not that a save
/// is taking long, and once the device is back it syncs with no error left over.
#[test]
#[ignore = "while a seal's publish waits on a silent server, the text module holds new edits out of the journal and stays unsettled"]
fn an_edit_made_offline_reads_as_kept_on_the_device_and_syncs_without_an_error() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut phone = HostTest::device(&dev);
    add_to_my_files(&mut phone, "Text");
    phone.click("#text.surface");
    phone.type_text("Recipes");
    phone.synced();

    // Sealing waits for a pause in typing, as for a person, so the second edit comes while the
    // first one's seal is out to a server that no longer answers.
    phone.run(&["hurry", "off"]);
    phone.unreachable();
    phone.run(&["--no-settle", "type", " for soup"]);
    phone.pause(5_000);
    phone.run(&["--no-settle", "type", " and bread"]);
    phone.pause(10_000);
    assert!(
        !phone.shown("Saving changes"),
        "an edit kept on the device is saved, offline or not"
    );
    phone.online();
    phone.synced();
    phone.restart();
    phone.synced();
    assert!(
        !phone.tree().contains("could not sync"),
        "nothing is left of the drop once the device synced"
    );
    phone.close();
}
