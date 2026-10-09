use std::time::Duration;

use beui::Rect;
use beui::icons::ICON_VOLUME_UP;
use beui::reactive::{Frame, List, NodeRef, build, create_signal, view};
use beui::styled::{Button, ButtonVariant, LevelOsd, OSD_DURATION, OsdLevel, use_theme};

use super::*;

fn level_display(screens: Vec<Rect>) -> DocumentTest {
    let document = build(move || {
        let theme = use_theme();
        let area = NodeRef::new();
        let (shown, set_shown) = create_signal(0_u64);
        let level = Some(OsdLevel {
            glyph: ICON_VOLUME_UP.to_owned(),
            label: "Volume".to_owned(),
            level: 0.65,
            muted: false,
        });
        view! {
            <Frame @node_ref=&area color={theme.background.clone()}>
                <List spacing=0.0>
                    <Button
                        @test_id={"test.show"}
                        label="Turn it up"
                        variant=ButtonVariant::Secondary
                        on_click={move || set_shown.update(|shown| *shown += 1)}
                    />
                    <LevelOsd anchor={area.clone()} level={level} shown={shown} screens={screens} />
                </List>
            </Frame>
        }
    });
    DocumentTest::new(document, Vec2::new(1100.0, 480.0))
}

#[test]
fn the_level_display_shows_on_a_change_and_fades_out() {
    let mut test = level_display(Vec::new());
    test.frame(Vec::new());
    assert!(!test.shows("level-osd.0"), "nothing shows before a change");

    test.click("test.show");
    test.frame(Vec::new());
    assert!(test.shows("level-osd.0"), "a change shows the level");
    test.snapshot("level_display");

    test.advance(OSD_DURATION - Duration::from_millis(100));
    assert!(test.shows("level-osd.0"), "the level stays up for a while");

    test.click("test.show");
    test.advance(OSD_DURATION - Duration::from_millis(100));
    assert!(
        test.shows("level-osd.0"),
        "another change keeps the level up for as long again"
    );

    test.advance(Duration::from_millis(200));
    assert!(
        test.shows("level-osd.0"),
        "the level fades rather than vanishing"
    );
    test.snapshot("level_display_fading");

    test.advance(Duration::from_secs(1));
    test.frame(Vec::new());
    assert!(
        !test.shows("level-osd.0"),
        "the level is gone once it has faded"
    );
}
