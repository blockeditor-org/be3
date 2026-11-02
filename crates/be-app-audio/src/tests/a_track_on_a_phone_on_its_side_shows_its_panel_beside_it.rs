use super::*;
use be_sdk_beui::beui::Vec2;

#[test]
fn a_track_on_a_phone_on_its_side_shows_its_panel_beside_it() {
    let mut test =
        BeuiTest::<AudioRoot>::editing(Uuid::new_v4()).with_size(Vec2::new(844.0, 390.0));
    test.hold(
        None,
        AudioFile::from_file("song.flac", "audio/flac", vec![1, 2, 3]).unwrap(),
    );
    test.run();
    test.settle();

    assert!(
        test.shown("audio.play") && test.shown("audio.replace"),
        "a phone shows the track with its panel gathered beside it"
    );
    test.snapshot("a_track_on_a_phone_on_its_side_shows_its_panel_beside_it");
}
