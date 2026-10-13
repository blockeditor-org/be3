use block_editor_beui::beui::pos2;

use super::*;

#[test]
fn the_volume_popup_asks_the_host_for_a_level_a_mute_and_an_output() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.hear(0.45, false, SPEAKERS);
    fixture.test.click("desktop.volume");
    fixture.settle();
    fixture.test.take_actions::<MediaRequest>();

    fixture.test.click("desktop.volume.mute");
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<MediaRequest>(),
        vec![MediaRequest::SetMute(true)]
    );

    let track = fixture.test.rect_of("desktop.volume.level");
    fixture
        .test
        .click_at(pos2(track.left() + track.width() * 0.9, track.center().y));
    fixture.settle();
    let asked = fixture.test.take_actions::<MediaRequest>();
    assert!(
        matches!(asked[..], [MediaRequest::SetVolume(level)] if level > 0.8),
        "a click near the end of the slider turns the volume up: {asked:?}"
    );

    fixture.test.click("desktop.volume.output");
    fixture.settle();
    fixture.click_text("Headphones");
    assert_eq!(
        fixture.test.take_actions::<MediaRequest>(),
        vec![MediaRequest::ChooseOutput(HEADPHONES.to_owned())]
    );
}
