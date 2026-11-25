use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_makes_a_block_a_picker_creates() {
    // With the root in place of be-host's UI, the canvas's block picker makes a new block from a
    // template: it places a creation child, which the root opens as the template's editor runs
    // it, and commits it; the host makes the block where the commit says, as be-host does, the
    // child's status says it was created, and the canvas places the new block.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Canvas");
    let block = host.place("#infinite-canvas.dock.block");
    let block = format!(
        "{:.0},{:.0}",
        block.0 + block.2 / 2.0,
        block.1 + block.3 / 2.0
    );
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.run(&["click", &block]);
    host.run(&["settle"]);
    // The picker's first row, the Canvas template (the type last made), where the picker hangs
    // over the Block button.
    host.run(&["click", "896,562"]);
    host.run(&["settle"]);
    // The files pane's Canvas row expanded: the new block is its child, by its name.
    host.run(&["click", "28,134"]);
    host.run(&["settle"]);
    host.snapshot("be_root_makes_a_block_a_picker_creates");
    host.close();
}
