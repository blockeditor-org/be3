use super::*;

#[test]
fn the_refresh_button_shows_on_an_epaper_display_and_asks_the_host_to_refresh() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.refresh"),
        "a screen that is not e-paper has nothing to refresh"
    );

    fixture.test.set_watched::<EpaperWatch>(&true);
    fixture.settle();
    assert!(fixture.test.shown("desktop.refresh"));
    fixture.test.snapshot("the_refresh_button_in_the_bar");

    fixture.test.click("desktop.refresh");
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::epaper_refresh),
        vec![()]
    );
}
