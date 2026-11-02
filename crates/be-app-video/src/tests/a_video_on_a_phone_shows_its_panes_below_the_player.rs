use super::*;

#[test]
fn a_video_on_a_phone_shows_its_panes_below_the_player() {
    let mut fixture = Fixture::sized(Vec2::new(390.0, 844.0));
    fixture.editor.run();
    let clip = fixture.insert(0);
    fixture.insert(1);
    fixture.editor.run();
    fixture.editor.settle();

    assert!(
        fixture.editor.shown("video.playback")
            && fixture.editor.shown("video.timeline")
            && fixture.editor.shown("video.timeline-tools"),
        "a phone shows the player with the timeline and its tools gathered below it"
    );
    fixture.editor.record();

    fixture.editor.tap(format!("video.clip.{clip}"));
    fixture.editor.run();
    fixture.editor.tap("dock.tab.effects");
    fixture.editor.run();
    fixture.editor.settle();
    assert!(
        fixture.editor.shown("video.clip-length"),
        "the effects tab shows the clip tapped in the timeline"
    );
    fixture.editor.record();
    fixture
        .editor
        .snapshot("a_video_on_a_phone_shows_its_panes_below_the_player");
}
