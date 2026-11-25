use super::*;

const CONFIRMATION: &str = "#demo.discard";

/// A confirmation blocks the frame that asked and no other: the files pane, another editor's
/// frame beside it, still takes presses and typing while it is open, and the confirmation stays:
/// it is inside its region, not a small form a press outside closes.
#[test]
fn a_confirmation_in_one_editor_leaves_the_files_beside_it_live() {
    let mut host = HostTest::start(Options::default());
    open_beui_demo(&mut host);
    host.click("#demo.catalog.Overlays");
    host.wait("#demo.discard.open");
    host.click("#demo.discard.open");
    host.wait(CONFIRMATION);
    host.run(&["settle"]);

    host.click("#file-tree.filter");
    host.type_text("quince");
    host.run(&["settle"]);
    assert!(
        host.shown("quince"),
        "the files beside the confirmation take the press and the typing"
    );
    host.wait(CONFIRMATION);
    host.snapshot("a_confirmation_in_one_editor_leaves_the_files_beside_it_live");
    host.close();
}
