use super::*;

/// On a phone a new block opens in the files' place, so its row in the files is not left waiting
/// for a name: back from it, one tap on its row opens it again.
#[test]
fn on_a_phone_a_new_text_opens_from_its_row_with_one_tap_after_back() {
    let mut host = HostTest::start(Options::default());
    host.run(&["resize", "390x844"]);
    open_add_to_my_files(&mut host);
    // On a phone the picker is a sheet, which slides up on the clock before its rows are where
    // a tap finds them, and away again after.
    host.wait("#picker.more");
    host.pause(600);
    host.run(&["tap", "MenuItem \"Text\""]);
    host.pause(600);
    host.wait("Pane \"Text\"");
    host.wait("MultilineTextInput focused");
    host.run(&["back"]);
    host.pause(600);
    host.gone("Pane \"Text\"");
    host.wait("TreeItem \"Text\"");
    assert!(
        !host.shown("TextInput \"Name\""),
        "no name field waits in the files"
    );
    host.run(&["tap", "TreeItem \"Text\""]);
    host.wait("Pane \"Text\"");
    host.close();
}
