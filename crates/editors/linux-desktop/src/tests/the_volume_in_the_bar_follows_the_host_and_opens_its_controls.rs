use block_editor_beui::beui::reactive::Get;
use block_editor_beui::beui::styled::SliderApi;
use block_editor_beui::beui::styled::ToggleApi;

use super::*;

#[test]
fn the_volume_in_the_bar_follows_the_host_and_opens_its_controls() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.volume"),
        "there is no volume to show before the host reports one"
    );

    fixture.hear(0.45, false, SPEAKERS);
    assert!(fixture.test.shown("desktop.volume"));
    assert!(fixture.test.label("desktop.volume").contains("45%"));
    fixture.test.snapshot("the_volume_in_the_bar");

    fixture.test.click("desktop.volume");
    fixture.settle();
    assert!(fixture.test.shown("desktop.volume.panel"));
    assert!(
        fixture
            .test
            .label("desktop.volume.output")
            .contains("Speakers"),
        "the output in use is chosen"
    );
    fixture.test.snapshot("the_volume_popup");

    fixture.hear(0.7, true, HEADPHONES);
    assert!(
        fixture.test.label("desktop.volume").contains("70%"),
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
        !fixture.test.shown("desktop.volume"),
        "the item goes when the sound server does"
    );
}
