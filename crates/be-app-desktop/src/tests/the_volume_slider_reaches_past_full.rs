use be_sdk_beui::beui::pos2;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::SliderApi;

use super::*;

#[test]
fn the_volume_slider_reaches_past_full() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.hear(1.2, false, SPEAKERS);
    fixture.test.click("desktop.status");
    fixture.settle();
    fixture.test.take_host_calls(be_sdk_test::requested::media);

    let document = fixture.test.document();
    let slider = document.find_test_id("desktop.volume.level").unwrap();
    assert!((document.api::<SliderApi>(slider).value().get() - 1.2).abs() < 1e-4);
    drop(document);
    assert!(fixture.says("120%"));
    fixture.test.snapshot("the_volume_slider_past_full");

    let track = fixture.test.rect_of("desktop.volume.level");
    fixture
        .test
        .click(pos2(track.right() - 1.0, track.center().y));
    fixture.settle();
    let asked = fixture.test.take_host_calls(be_sdk_test::requested::media);
    assert!(
        matches!(asked[..], [MediaRequest::SetVolume(level)] if (level - MediaLevel::LOUDEST).abs() < 0.01),
        "the end of the slider is the loudest the volume goes: {asked:?}"
    );
}
