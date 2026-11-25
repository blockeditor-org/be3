use super::*;

/// A block dragged from Recently Deleted onto the My files row moves into My files, not to the
/// root.
#[test]
fn a_deleted_block_dragged_onto_my_files_moves_into_it() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    let more = row_menu(&mut host, "Text");
    host.click(&more);
    host.click("MenuItem \"Delete\"");
    host.wait("#workspace.deleted.restore");
    // Zooming into Recently Deleted unfolds it, so it stays open once the files zoom out.
    host.click("TreeItem \"Recently Deleted\"");
    host.wait("TreeItem \"Text\"");
    host.click("#file-tree.back");
    host.wait("TreeItem \"My files\"");
    let row = host.place("TreeItem \"Text\"");
    let target = host.place("TreeItem \"My files\"");
    let start = (row.0 + row.2 / 2.0, row.1 + row.3 / 2.0);
    let to = (target.0 + target.2 / 2.0, target.1 + target.3 / 2.0);
    host.run(&["down", "TreeItem \"Text\""]);
    host.run(&["move", &format!("{},{}", start.0 + 12.0, start.1)]);
    host.run(&["pause", "300"]);
    for step in 1..=4 {
        let share = step as f32 / 4.0;
        let x = start.0 + (to.0 - start.0) * share;
        let y = start.1 + (to.1 - start.1) * share;
        host.run(&["move", &format!("{x},{y}")]);
        host.run(&["pause", "100"]);
    }
    host.run(&["up"]);
    host.gone("#workspace.deleted.restore");
    host.wait("TreeItem \"No recently deleted blocks\"");
    let (mine, text, unfiled) = (
        host.place("TreeItem \"My files\""),
        host.place("TreeItem \"Text\""),
        host.place("TreeItem \"Unfiled\""),
    );
    assert!(
        mine.1 < text.1 && text.1 < unfiled.1,
        "the block is listed in My files, between it and Unfiled"
    );
    host.close();
}
