use super::*;

#[test]
fn a_beui_demos_dock_joins_the_workspace_dock_as_a_locked_group() {
    let mut app = AppTest::start();
    open_beui_demo(&mut app);
    app.snapshot("a_beui_demos_dock_joins_the_workspace_dock_as_a_locked_group");
    app.close();
}
