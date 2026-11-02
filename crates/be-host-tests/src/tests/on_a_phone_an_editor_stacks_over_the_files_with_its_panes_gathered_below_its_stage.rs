use super::*;

#[test]
fn on_a_phone_an_editor_stacks_over_the_files_with_its_panes_gathered_below_its_stage() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Pan and Zoom");
    host.wait("#pan_zoom.card.0");
    host.gone("#editor.loading");
    host.run(&["resize", "390x844"]);
    host.run(&["settle"]);
    host.wait("#dock.back");
    host.wait("#dock.compact");
    host.wait("#pan_zoom.focus.0");
    let stage = host.place("#pan_zoom.stage");
    let gathered = host.place("#dock.compact");
    assert!(
        stage.1 + stage.3 <= gathered.1 + 1.0 && gathered.2 > 380.0,
        "the editor's cards are gathered below its stage across the phone, {stage:?} {gathered:?}"
    );
    let world = pan_zoom_world(&mut host);
    assert!(
        world.0 >= stage.0 - 1.0 && world.0 + world.2 <= stage.0 + stage.2 + 1.0,
        "the editor keeps its cards fitted into its stage, {world:?} in {stage:?}"
    );
    host.snapshot("on_a_phone_an_editor_stacks_over_the_files_with_its_panes_gathered_below_its_stage");
    host.close();
}
