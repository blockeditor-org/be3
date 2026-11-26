use super::restores::*;
use super::*;
use be_sdk_api::{DockAlign, DockBarEdge, DockBarPlacement, DockEdge, DockUpdate};

#[test]
fn a_restored_bar_withdrawn_before_it_lands_does_not_stop_saving() {
    let moved = DockBarPlacement::Beside(DockBarEdge {
        edge: DockEdge::Left,
        align: DockAlign::Center,
    });
    let Rebuilt {
        mut test,
        display,
        stage,
        bar,
        ..
    } = rebuilt(moved);

    // The bar is withdrawn before the root takes the restore: the root never reports it.
    BAR.with(|flag| flag.get().unwrap().set(false));
    test.run();
    test.take_dock_messages();
    // The root reports the stage alone, then the person adds a side pane: a change of their own.
    let mut only_stage = hosted(stage, bar, moved, false, 1);
    let mut with_side = hosted(stage, bar, moved, true, 1);
    for update in [&mut only_stage, &mut with_side] {
        if let DockUpdate::Hosted(view) = update {
            view.hosted.as_mut().unwrap().layout.bars.clear();
        }
    }
    test.receive_dock_updates(vec![only_stage]);
    let before = kept(&test, display);
    test.receive_dock_updates(vec![with_side]);
    assert_ne!(
        kept(&test, display),
        before,
        "a change the person makes after the restored bar is withdrawn is kept"
    );
}
