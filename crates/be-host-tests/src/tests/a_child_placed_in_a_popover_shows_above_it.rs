use super::*;

/// A child an app places above its own popover shows over it (the Debug view's Preview, which
/// holds the block in its own editor).
#[test]
fn a_child_placed_in_a_popover_shows_above_it() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    host.click("#text.surface");
    host.type_text("Shown above the popover");
    host.wait("TreeItem \"Shown above the popover\"");
    host.run(&["click", "TreeItem \"Shown above the popover\"", "right"]);
    host.click("#file-tree.open-with");
    host.click("#file-tree.open-with.777b1dfa-5369-402e-b22d-7eb56877ad4d");
    host.wait("#block-debug.preview");
    host.click("#block-debug.preview");
    host.wait("#block-debug.preview.child");
    host.wait("Pane \"Shown above the popover\"");
    host.crop("#workspace.block");
    host.snapshot("a_child_placed_in_a_popover_shows_above_it");
    host.close();
}
