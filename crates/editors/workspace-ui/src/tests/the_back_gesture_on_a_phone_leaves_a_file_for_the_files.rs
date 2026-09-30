use super::*;

#[test]
fn the_back_gesture_on_a_phone_leaves_a_file_for_the_files() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(390.0, 800.0)));
    assert!(
        !fixture.test.handles_back(),
        "the files page has nothing to go back to"
    );

    show(&mut fixture, opened, None);
    assert_eq!(fixture.shown(), vec![opened]);
    assert!(
        fixture.test.handles_back(),
        "a file on show tells the host it can go back"
    );

    fixture.test.back();
    fixture.settle();
    assert!(
        fixture.shown().is_empty(),
        "going back leaves the file for the files page"
    );
    assert!(!fixture.test.handles_back());
}
