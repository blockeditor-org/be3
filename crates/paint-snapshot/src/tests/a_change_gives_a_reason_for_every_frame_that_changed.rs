use super::*;

#[test]
fn a_change_gives_a_reason_for_every_frame_that_changed() {
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    let change = crate::Change {
        name: "editor.a_recording".to_owned(),
        before: Some(triangles(&[red, red, red])),
        after: Some(triangles(&[blue, red, blue])),
    };

    let reasons = change.reasons();
    assert_eq!(reasons.len(), 2);
    assert!(reasons[0].starts_with("frame 1 of 3 changed:"));
    assert!(reasons[1].starts_with("frame 3 of 3 changed:"));
}
