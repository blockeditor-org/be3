use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_opens_the_share_dialog_an_editor_asks_for() {
    // With the root in place of be-host's UI, an editor's Share (its tab's pinned action, a
    // `share` command) opens the share dialog in the session's editor the root embeds, as
    // be-host's own shell opens it, and its field takes what is typed once pressed.
    let server = server();
    let dev = server.account("Dev");
    let mut host = HostTest::root_device(&dev, ROOT);
    host.run(&["settle"]);
    host.click(NEW);
    host.click(NEW_IN_MY_FILES);
    host.click("#picker.more");
    host.click("Button \"Chat\"");
    host.wait("TextInput \"Name\" value=\"Chat\"");
    host.key("Enter");
    host.wait("TreeItem \"Chat\"");
    host.click("#dock.editors.pin.editor.share");
    host.wait("#share.query");
    host.click("#share.query");
    // The field the press focused, in the dialog the workspace anchored, takes what is typed
    // and holds the window's focus, as under be-host.
    host.type_text("Ada");
    host.wait("TextInput \"Add contacts\" value=\"Ada\" focused");
    host.run(&["settle"]);
    host.snapshot("be_root_opens_the_share_dialog_an_editor_asks_for");
    host.close();
}
