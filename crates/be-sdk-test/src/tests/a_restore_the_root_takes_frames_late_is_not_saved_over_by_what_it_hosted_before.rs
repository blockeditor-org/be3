use super::restores::*;
use be_sdk_api::{DockAlign, DockBarEdge, DockBarPlacement, DockEdge};

/// The root takes a restore some frames after it is asked, reporting meanwhile the dock as it
/// hosted each declaration it heard, with the bar where the declaration puts it. None of that is
/// kept over the saved layout, whether the restore moved the bar (beside the stage) or left it
/// where the dock declares it (over its top), so that what the root hosted before looks the
/// same as what the restore asks for.
#[test]
fn a_restore_the_root_takes_frames_late_is_not_saved_over_by_what_it_hosted_before() {
    let beside = DockBarPlacement::Beside(DockBarEdge {
        edge: DockEdge::Left,
        align: DockAlign::Center,
    });
    for moved in [beside, over(DockEdge::Top, DockAlign::Center)] {
        let Rebuilt {
            mut test,
            display,
            saved,
            stage,
            bar,
        } = rebuilt(moved);

        let declared_at = over(DockEdge::Top, DockAlign::Center);
        test.receive_dock_updates(vec![hosted(stage, bar, declared_at, true, 0)]);
        assert_eq!(
            kept(&test, display),
            saved,
            "a layout the root hosted before it took the restore is not kept ({moved:?})"
        );
        test.receive_dock_updates(vec![hosted(stage, bar, declared_at, false, 0)]);
        assert_eq!(
            kept(&test, display),
            saved,
            "no layout the root hosted before it took the restore is kept over the saved one \
             ({moved:?})"
        );

        test.receive_dock_updates(vec![hosted(stage, bar, moved, false, 1)]);
        assert_eq!(
            kept(&test, display),
            saved,
            "the restored layout is the one kept ({moved:?})"
        );

        let dragged = over(DockEdge::Bottom, DockAlign::Start);
        test.receive_dock_updates(vec![hosted(stage, bar, dragged, false, 1)]);
        assert_ne!(
            kept(&test, display),
            saved,
            "once restored, a bar moved again is kept ({moved:?})"
        );
    }
}
