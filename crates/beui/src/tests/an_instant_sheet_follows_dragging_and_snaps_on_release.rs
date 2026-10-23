use super::*;
use crate::{PreferenceLayer, PreferenceSource};

#[test]
fn an_instant_sheet_follows_dragging_and_snaps_on_release() {
    let _root = crate::reactive::RootScope::new();
    let sheet = NodeRef::new();
    let closed = create_signal(0);
    let document = sheet_of_rows(0.5, sheet, closed);
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Instant),
            ..PreferenceLayer::EMPTY
        },
    );
    harness.settle();
    let sheet = sheet.get();
    let height = |harness: &Harness| harness.rect(sheet).height();

    harness.finger_drag_and_hold(pos2(200.0, 450.0), pos2(200.0, 270.0));
    assert_eq!(
        height(&harness),
        540.0,
        "the sheet snaps immediately on release"
    );

    let from = pos2(200.0, 200.0);
    harness.finger_drag(&[
        from,
        from + vec2(0.0, 40.0),
        from + vec2(0.0, 80.0),
        from + vec2(0.0, 120.0),
    ]);
    assert_eq!(
        closed.get_untracked(),
        1,
        "a quick flick down closes the sheet, though it was let go high up"
    );
    harness.settle();
    assert_eq!(
        height(&harness),
        300.0,
        "a sheet its owner keeps open springs back to where it rests"
    );
}
