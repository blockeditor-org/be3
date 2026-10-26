use super::*;

#[test]
fn the_level_display_shows_on_a_change_and_fades_out() {
    let mut test = level_display();
    test.frame(Vec::new());
    assert!(!test.shows("level-osd"), "nothing shows before a change");

    test.click("test.show");
    test.frame(Vec::new());
    assert!(test.shows("level-osd"), "a change shows the level");
    test.snapshot("level_display");

    test.advance(OSD_DURATION - Duration::from_millis(100));

    test.frame(Vec::new());
    assert!(test.shows("level-osd"), "the level stays up for a while");

    test.click("test.show");
    test.advance(OSD_DURATION - Duration::from_millis(100));
    test.frame(Vec::new());
    assert!(
        test.shows("level-osd"),
        "another change keeps the level up for as long again"
    );

    test.advance(Duration::from_millis(200));

    test.frame(Vec::new());
    assert!(
        test.shows("level-osd"),
        "the level fades rather than vanishing"
    );
    test.snapshot("level_display_fading");

    test.advance(Duration::from_secs(1));

    test.frame(Vec::new());
    test.frame(Vec::new());
    assert!(
        !test.shows("level-osd"),
        "the level is gone once it has faded"
    );
}
