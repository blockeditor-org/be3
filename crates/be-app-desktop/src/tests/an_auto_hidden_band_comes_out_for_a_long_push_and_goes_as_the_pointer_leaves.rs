use super::*;

#[test]
fn an_auto_hidden_band_comes_out_for_a_long_push_and_goes_as_the_pointer_leaves() {
    let mut fixture = Fixture::with_windows(&[1]);
    let band = fixture.auto_hidden();
    let thick = band.height();
    assert!(
        !fixture.test.shown("desktop.bar.status") && !fixture.test.shown("desktop.apps"),
        "the auto-hidden band is gone from the foot"
    );
    assert_eq!(
        fixture.sent_input(),
        ScreenInput {
            walls: vec![ScreenWall {
                id: 1,
                display: SCREEN,
                side: ScreenSide::Bottom,
                start: 0.0,
                end: 800.0,
            }],
            regions: vec![ScreenRegion {
                id: FOOT_SWIPE,
                rect: Rect::from_min_max(pos2(0.0, 594.0), pos2(800.0, 600.0)),
                captures_touch: true,
            }],
        },
        "the host walls the pointer along the foot and gives the desktop fingers landing by it"
    );

    fixture.screen_input(at_foot(thick * 3.0, true, false));
    fixture.wait(16);
    assert!(
        !fixture.test.shown("desktop.bar.status"),
        "a push as the pointer arrives at the foot, as a sweep that ends there makes, shows nothing"
    );

    fixture.rest_on_foot(thick * 3.0);
    fixture.screen_input(at_foot(thick * 3.9, true, false));
    fixture.wait(16);
    assert!(
        !fixture.test.shown("desktop.bar.status"),
        "a push of less than the band's thickness shows nothing"
    );
    fixture.screen_input(at_foot(thick * 4.3, true, false));
    fixture.wait(16);
    assert!(
        fixture.test.shown("desktop.bar.status"),
        "pushing on brings the band partly out"
    );
    fixture
        .test
        .snapshot("an_auto_hidden_band_partly_pushed_out");
    fixture.wait(1000);
    assert!(
        !fixture.test.shown("desktop.bar.status"),
        "short of half, the band slides back once the push stops"
    );

    fixture.screen_input(at_foot(thick * 6.3, true, true));
    fixture.wait(500);
    let status = fixture.test.rect_of("desktop.bar.status");
    assert!(
        (status.bottom() - 600.0).abs() < 1.0,
        "past half, the band comes all the way out: {status:?}"
    );
    let open = fixture.sent_input();
    assert!(
        open.walls.is_empty()
            && open.regions.len() == 1
            && open.regions[0].id == FOOT_KEEP
            && open.regions[0].rect.contains_rect(band),
        "the open band lets the pointer through and asks whether it is near: {open:?}"
    );
    fixture.test.snapshot("an_auto_hidden_band_pushed_out");

    fixture.wait(1000);
    assert!(
        fixture.test.shown("desktop.bar.status"),
        "it stays while the pointer is near"
    );
    fixture.screen_input(at_foot(thick * 6.3, false, false));
    fixture.wait(50);
    let going = fixture.test.rect_of("desktop.bar.status");
    assert!(
        going.bottom() > 601.0,
        "it starts going the moment the pointer leaves: {going:?}"
    );
    let closing = fixture.sent_input();
    assert!(
        closing.walls.len() == 1 && closing.regions.iter().any(|region| region.id == FOOT_KEEP),
        "while it goes, the wall is back and the host still says whether the pointer is near: {closing:?}"
    );
    fixture.screen_input(at_foot(thick * 6.3, false, true));
    fixture.wait(500);
    let status = fixture.test.rect_of("desktop.bar.status");
    assert!(
        (status.bottom() - 600.0).abs() < 1.0,
        "the pointer coming back before it is gone brings it back out: {status:?}"
    );

    fixture.screen_input(at_foot(thick * 6.3, false, false));
    fixture.wait(500);
    assert!(
        !fixture.test.shown("desktop.bar.status"),
        "and once the pointer leaves for good, it is gone"
    );
    let gone = fixture.sent_input();
    assert!(
        gone.walls.len() == 1 && gone.regions.iter().all(|region| region.id != FOOT_KEEP),
        "the wall is back, and nothing asks after the pointer: {gone:?}"
    );
}
