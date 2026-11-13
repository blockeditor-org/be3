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

    for _ in 0..HeldChanges::KEPT + 4 {
        held.hold(key(), ViewChange::Pan(Vec2::new(1.0, 0.0)));
    }
    held.hold(key(), ViewChange::Fit);
    let kept = held.take(&key());
    assert_eq!(
        kept.len(),
        HeldChanges::KEPT,
        "an instance no camera shows keeps only its latest few"
    );
    assert_eq!(kept.last(), Some(&ViewChange::Fit));
}
