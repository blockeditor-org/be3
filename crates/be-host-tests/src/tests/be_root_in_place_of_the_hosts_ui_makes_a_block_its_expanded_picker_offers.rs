use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

const NAME: &str = "TextInput \"Name\" value=\"Chat\"";

#[test]
fn be_root_in_place_of_the_hosts_ui_makes_a_block_its_expanded_picker_offers() {
    // With the root in place of be-host's UI, the files' New menu's picker grows into its card
    // of every block type, which shows at its new size as be-host's does (the picker fits itself
    // to the card, and the frame that draws it at that size follows), and the type pressed there
    // is made in My files: a chat, which the files offer to name.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.click(NEW);
    host.click(NEW_IN_MY_FILES);
    host.click("#picker.more");
    host.run(&["settle"]);
    host.snapshot("be_root_shows_its_pickers_card_of_every_type");
    host.click("Button \"Chat\"");
    host.wait(NAME);
    host.key("Enter");
    host.gone(NAME);
    host.wait("TreeItem \"Chat\"");
    host.close();
}
