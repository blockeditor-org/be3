use super::*;

fn key() -> Key {
    ("app".to_owned(), EditorInstanceId(1))
}

#[test]
fn a_view_change_waits_for_its_camera_and_goes_when_its_instance_closes() {
    let mut held = HeldChanges::default();
    held.hold(key(), ViewChange::Fit);
    assert_eq!(
        held.take(&key()),
        [ViewChange::Fit],
        "a change asked before its camera is shown waits for it"
    );
    assert!(held.take(&key()).is_empty(), "and is taken once");

    held.hold(key(), ViewChange::Fit);
    held.forget(&key());
    assert!(
        held.take(&key()).is_empty(),
        "nothing is kept once the instance closes"
    );

    held.hold(key(), ViewChange::KeepFitted(None));
    held.hold(key(), ViewChange::Pan(Vec2::new(5.0, 0.0)));
    held.hold(key(), ViewChange::Fit);
    for _ in 0..20 {
        held.hold(key(), ViewChange::Pan(Vec2::new(1.0, 0.0)));
    }
    assert_eq!(
        held.take(&key()),
        [
            ViewChange::KeepFitted(None),
            ViewChange::Fit,
            ViewChange::Pan(Vec2::new(20.0, 0.0)),
        ],
        "keep-fitted survives any number of moves, a fit clears the moves before it, and \
         pans add up"
    );

    for factor in [2.0, 3.0] {
        held.hold(
            key(),
            ViewChange::Zoom {
                factor,
                anchor: None,
            },
        );
    }
    assert_eq!(
        held.take(&key()),
        [ViewChange::Zoom {
            factor: 6.0,
            anchor: None
        }],
        "zooms about one anchor multiply"
    );
}
