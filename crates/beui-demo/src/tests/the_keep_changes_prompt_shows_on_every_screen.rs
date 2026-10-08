use beui::Rect;
use beui::reactive::{Frame, build, view};
use beui::styled::{KeepChanges, use_theme};

use super::*;

#[test]
fn the_keep_changes_prompt_shows_on_every_screen() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(640.0, 480.0));
    let right = Rect::from_min_size(pos2(640.0, 0.0), Vec2::new(460.0, 320.0));
    let document = build(move || {
        let theme = use_theme();
        view! {
            <Frame color={theme.background.clone()}>
                <KeepChanges
                    open=true
                    title="Keep these display settings?"
                    screens={vec![left, right]}
                    on_keep={|| {}}
                    on_revert={|| {}}
                />
            </Frame>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(1100.0, 480.0));
    test.frame(Vec::new());
    assert!(left.contains(test.rect_of("keep-changes.0").center()));
    assert!(right.contains(test.rect_of("keep-changes.1").center()));
    test.snapshot("keep_changes_on_two_screens");
}
