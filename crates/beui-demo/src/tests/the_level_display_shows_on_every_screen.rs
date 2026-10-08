use beui::Rect;
use beui::icons::ICON_VOLUME_OFF;
use beui::reactive::{Frame, NodeRef, build, view};
use beui::styled::{LevelOsd, OsdLevel, use_theme};

use super::*;

#[test]
fn the_level_display_shows_on_every_screen() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(640.0, 480.0));
    let right = Rect::from_min_size(pos2(640.0, 0.0), Vec2::new(460.0, 320.0));
    let document = build(move || {
        let theme = use_theme();
        let area = NodeRef::new();
        let level = Some(OsdLevel {
            glyph: ICON_VOLUME_OFF.to_owned(),
            label: "Volume".to_owned(),
            level: 0.3,
            muted: true,
        });
        view! {
            <Frame @node_ref=&area color={theme.background.clone()}>
                <LevelOsd
                    anchor={area.clone()}
                    level={level}
                    shown=1_u64
                    screens={vec![left, right]}
                />
            </Frame>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(1100.0, 480.0));
    test.frame(Vec::new());
    assert!(left.contains(test.rect_of("level-osd.0").center()));
    assert!(right.contains(test.rect_of("level-osd.1").center()));
    test.snapshot("level_display_muted_on_two_screens");
}
