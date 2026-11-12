use super::*;

/// Offline, an edit is saved on the device and waits to sync: the host says so, not that a save
/// is taking long, and once the device is back it syncs with no error left over.
#[test]
fn an_edit_made_offline_reads_as_kept_on_the_device_and_syncs_without_an_error() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut phone = HostTest::device(&dev);
    add_to_my_files(&mut phone, "Text");
    phone.click("#text.surface");
    phone.type_text("Recipes");
    phone.synced();

    // A driven host seals each edit at once, so the second edit comes while the first one's seal
    // waits on a server that no longer answers. The clock runs, as for a person, for the
    // "Saving changes" toast, which shows once edits stay off the device for a few seconds.
    phone.run(&["clock", "run"]);
    phone.unreachable();
    phone.run(&["--no-settle", "type", " for soup"]);
    phone.run(&["--no-settle", "type", " and bread"]);
    phone.run(&["--no-settle", "pause", "6000"]);
    let tree = phone.run(&["--no-settle", "tree"]);
    assert!(
        !tree.contains("Saving changes"),
        "an edit kept on the device is saved, offline or not:\n{tree}"
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
