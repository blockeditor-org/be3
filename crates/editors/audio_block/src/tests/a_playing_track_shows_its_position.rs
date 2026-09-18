use super::*;

#[test]
fn a_playing_track_shows_its_position() {
    let (mut test, editor, _block) = editor();

    editor.host().set_audio(AudioStatus {
        playing: true,
        position_micros: 65_000_000,
        duration_micros: Some(200_000_000),
        error: None,
    });
    test.run();

    assert_eq!(test.label("audio.position"), "1:05 / 3:20");
    test.snapshot("a_playing_track_shows_its_position");
}
