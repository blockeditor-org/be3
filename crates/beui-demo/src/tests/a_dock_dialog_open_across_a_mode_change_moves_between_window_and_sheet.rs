use std::time::Duration;

use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);

fn settle(test: &mut DocumentTest) {
    for _ in 0..30 {
        test.advance(Duration::from_millis(16));
        test.frame(Vec::new());
    }
}

#[test]
fn a_dock_dialog_open_across_a_mode_change_moves_between_window_and_sheet() {
    let mut test = demo(WIDE);
    reveal(&mut test, "demo.dialog.open");
    test.click("demo.dialog.open");
    test.frame(Vec::new());
    let dialog = dialog_id(0);
    let (header, sheet) = (format!("{dialog}.header"), format!("{dialog}.sheet"));
    assert!(test.shown(&header) && test.shown(&dialog));

    test.resize(PHONE);
    settle(&mut test);
    assert!(
        test.shown(&sheet) && test.shown(&dialog) && !test.shown(&header),
        "narrowed to a phone, the open dialog's body shows in a sheet with no window bar"
    );

    test.resize(WIDE);
    settle(&mut test);
    assert!(
        test.shown(&header) && test.shown(&dialog) && !test.shown(&sheet),
        "widened again, it is a window with its bar and body"
    );
}
