use super::*;

#[test]
fn an_empty_video_has_nothing_at_the_playhead() {
    let mut fixture = editor();

    assert_eq!(fixture.video().duration(), 0);
    fixture
        .editor
        .snapshot("an_empty_video_has_nothing_at_the_playhead");
}
