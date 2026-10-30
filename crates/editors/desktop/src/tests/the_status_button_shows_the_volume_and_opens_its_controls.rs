use block_editor_beui::beui::reactive::Get;
use block_editor_beui::beui::styled::SliderApi;
use block_editor_beui::beui::styled::ToggleApi;

use super::*;

#[test]
fn the_status_button_shows_the_volume_and_opens_its_controls() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.status.volume"),
        "there is no volume to show before the host reports one"
    );

    fixture.hear(0.45, false, SPEAKERS);
    fixture.wifi(nearby());
    fixture.allow_power(EVERYTHING);
    assert!(fixture.test.shown("desktop.status.volume"));
    assert!(
        fixture.status_label().contains("45%"),
        "the status button names the volume: {}",
        fixture.status_label()
    );
    fixture.test.snapshot("the_status_in_the_bar");

    fixture.test.click("desktop.status");
    fixture.settle();
    assert!(fixture.test.shown("desktop.volume.panel"));
    assert!(
        fixture
            .test
            .label("desktop.volume.output")
            .contains("Speakers"),
        "the output in use is chosen"
    );

    fixture.hear(0.7, true, HEADPHONES);
    assert!(
        fixture.status_label().contains("70%"),
        "a change made by the volume keys or another program shows in the bar"
    );
    let document = fixture.test.document();
    let slider = document.find_test_id("desktop.volume.level").unwrap();
    let mute = document.find_test_id("desktop.volume.mute").unwrap();
    assert!((document.api::<SliderApi>(slider).value().get() - 0.7).abs() < 1e-4);
    assert!(
        document.api::<ToggleApi>(mute).checked().get(),
        "and in the open popup"
    );
    drop(document);
    assert!(
        fixture
            .test
            .label("desktop.volume.output")
            .contains("Headphones"),
        "the output another program chose is the one shown"
    );

    fixture
        .test
        .set_watched::<MediaWatch>(&MediaLevels::default());
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.status.volume") && !fixture.test.shown("desktop.volume.panel"),
        "the volume goes when the sound server does"
    );
}
