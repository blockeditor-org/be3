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
    // The dialog grows and centres itself anew as who has access arrives from the server,
    // which settling does not wait for: its field is pressed once it has come to rest.
    host.wait("Label value=\"Can manage, from My files\"");
    host.run(&["settle"]);
    host.click("#share.query");
    // The field the press focused, in the dialog the workspace anchored, holds the window's
    // focus, as under be-host, in the tree the root publishes once the press settled: the
    // press changed no pixel the root draws but the tree, which it owes a frame.
    let tree = host.tree();
    assert!(
        tree.contains("TextInput \"Add contacts\" focused"),
        "the pressed field is focused once the press settled:\n{tree}"
    );
    host.type_text("Ada");
    host.wait("TextInput \"Add contacts\" value=\"Ada\" focused");
    host.run(&["settle"]);
    host.snapshot("be_root_opens_the_share_dialog_an_editor_asks_for");
    host.close();
}
