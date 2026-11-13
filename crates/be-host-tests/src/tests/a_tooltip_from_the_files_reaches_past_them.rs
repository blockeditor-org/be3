use super::*;

#[test]
fn a_tooltip_from_the_files_reaches_past_them() {
    let mut host = HostTest::start(Options::default());
    host.wait("#file-tree.shared.more");
    host.run(&["move", "#file-tree.shared.more"]);
    host.run(&["pause", "1500"]);
    let (files_x, _, files_width, _) = host.place("#workspace.files");
    let (tip_x, _, tip_width, _) = host.place("#tooltip");
    assert!(
        tip_x + tip_width > files_x + files_width,
        "the tooltip keeps its size and reaches past the files: {tip_x} + {tip_width}"
    );
    host.snapshot("a_tooltip_from_the_files_reaches_past_them");
    host.close();
}
