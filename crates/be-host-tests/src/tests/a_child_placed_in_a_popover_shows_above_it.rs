use super::*;

/// A child an app places above its own popover shows over it: the Debug view of a canvas
/// previews the canvas's first block, a text, in the text's own editor.
#[test]
fn a_child_placed_in_a_popover_shows_above_it() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Text");
    let tree = host.tree();
    let close = tree
        .lines()
        .skip_while(|line| !line.contains("Tab \"Canvas\""))
        .flat_map(str::split_whitespace)
        .find(|word| word.starts_with("#dock.tab.") && word.ends_with(".close"))
        .expect("the canvas tab has a close button")
        .to_owned();
    host.click(&close);
    host.gone("Tab \"Canvas\"");
    host.run(&["click", "TreeItem \"Canvas\"", "right"]);
    host.click("#file-tree.open-with");
    host.click("#file-tree.open-with.777b1dfa-5369-402e-b22d-7eb56877ad4d");
    host.wait("#block-debug.preview");
    host.click("#block-debug.preview");
    host.wait("#block-debug.preview.child");
    // Placed in the popover, over the Debug view. No painting: the view shows the blocks' ids,
    // which differ each run, even under the child.
    host.wait("Pane \"Text\" at 281,88 size 280x180");
    host.close();
}
