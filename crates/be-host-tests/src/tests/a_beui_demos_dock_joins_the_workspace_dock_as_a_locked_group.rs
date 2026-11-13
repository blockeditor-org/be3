use super::*;

#[test]
fn a_beui_demos_dock_joins_the_workspace_dock_as_a_locked_group() {
    let mut host = HostTest::start(Options::default());
    open_beui_demo(&mut host);
    host.snapshot("a_beui_demos_dock_joins_the_workspace_dock_as_a_locked_group");
    host.close();
}
