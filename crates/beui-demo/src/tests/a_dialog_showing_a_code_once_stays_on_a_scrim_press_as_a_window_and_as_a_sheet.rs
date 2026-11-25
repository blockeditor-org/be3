use super::*;

/// A dialog given `dismiss_on_outside_press=false` stays on a press on its scrim, as a centred
/// window and as a sheet in a narrow window alike, and closes from Done.
#[test]
fn a_dialog_showing_a_code_once_stays_on_a_scrim_press_as_a_window_and_as_a_sheet() {
    for size in [Vec2::new(320.0, 640.0), WIDE] {
        let mut test = alone(size, Page::Overlays);
        reveal(&mut test, "demo.once.open");
        test.click("demo.once.open");
        settle(&mut test);
        assert!(test.shown("demo.once.code"), "the code shows at {size:?}");

        test.click(pos2(size.x / 2.0, 8.0));
        settle(&mut test);
        assert!(
            test.shown("demo.once.code"),
            "a press on the scrim leaves it open at {size:?}"
        );

        test.click("demo.once.cancel");
        settle(&mut test);
        assert!(!test.shown("demo.once.code"), "Done closes it at {size:?}");
    }
}
