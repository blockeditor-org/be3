use super::*;
use beui::Key;

#[test]
fn a_docked_page_shown_fullscreen_covers_the_dock_until_escape() {
    let mut test = demo(WIDE);
    let page = pos2(WIDE.x * 0.75, WIDE.y / 2.0);
    for _ in 0..REVEAL_TICKS {
        if test.shown("demo.fullscreen") && test.rect_of("demo.fullscreen").bottom() <= WIDE.y {
            break;
        }
        test.scroll(page, Vec2::new(0.0, REVEAL_STEP));
    }
    let docked = test.rect_of("demo.fullscreen");

    test.click("demo.fullscreen");
    test.frame(Vec::new());
    let shown = test.rect_of("dock.fullscreen");
    assert_eq!(shown.size(), WIDE, "the page covers the whole window");
    assert!(
        test.rect_of("demo.fullscreen").left() < docked.left(),
        "its content moved out of the pane it sat in"
    );
    test.snapshot("docking_fullscreen");

    test.frame(
        [true, false]
            .into_iter()
            .map(|pressed| {
                Input::Key(beui::Keystroke {
                    key: Some(Key::Escape),
                    pressed,
                    ..Default::default()
                })
            })
            .collect(),
    );
    test.frame(Vec::new());
    assert!(
        !test.shown("dock.fullscreen"),
        "Escape brings the page back"
    );
    assert_eq!(test.rect_of("demo.fullscreen"), docked);
}
