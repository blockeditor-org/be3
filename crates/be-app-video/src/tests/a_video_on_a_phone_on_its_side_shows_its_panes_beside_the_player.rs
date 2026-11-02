use super::*;

#[test]
fn a_video_on_a_phone_on_its_side_shows_its_panes_beside_the_player() {
    let mut fixture = Fixture::sized(Vec2::new(844.0, 390.0));
    fixture.editor.run();
    let clip = fixture.insert(0);
    fixture.insert(1);
    fixture.editor.run();
    fixture.editor.tap(format!("video.clip.{clip}"));
    fixture.editor.run();
    fixture.editor.settle();

    assert!(
        fixture.editor.shown("video.playback")
            && fixture.editor.shown("video.timeline")
            && fixture.editor.shown("video.timeline-tools"),
        "a phone on its side shows the player with the timeline gathered beside it"
    );
    fixture
        .editor
        .snapshot("a_video_on_a_phone_on_its_side_shows_its_panes_beside_the_player");
}
