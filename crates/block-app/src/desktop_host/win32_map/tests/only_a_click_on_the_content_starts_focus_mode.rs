use beui::PointerId;

use super::*;

#[test]
fn only_a_click_on_the_content_starts_focus_mode() {
    let panel = Vec2::new(400.0, 300.0);
    let mut press = ContentPress::default();
    press.down(PointerId::Mouse, Pos2::new(100.0, 100.0));
    press.moved(PointerId::Mouse, Pos2::new(103.0, 102.0));
    assert_eq!(
        press.up(PointerId::Mouse, Pos2::new(104.0, 102.0), panel),
        Some(Pos2::new(104.0, 102.0))
    );
    assert_eq!(
        press.up(PointerId::Mouse, Pos2::new(104.0, 102.0), panel),
        None
    );

    press.down(PointerId::Mouse, Pos2::new(100.0, 100.0));
    press.moved(PointerId::Mouse, Pos2::new(160.0, 100.0));
    press.moved(PointerId::Mouse, Pos2::new(100.0, 100.0));
    assert_eq!(
        press.up(PointerId::Mouse, Pos2::new(100.0, 100.0), panel),
        None
    );

    press.down(PointerId::Mouse, Pos2::new(398.0, 100.0));
    assert_eq!(
        press.up(PointerId::Mouse, Pos2::new(402.0, 100.0), panel),
        None
    );

    press.down(PointerId::Mouse, Pos2::new(100.0, 100.0));
    assert_eq!(
        press.up(PointerId::Pen, Pos2::new(100.0, 100.0), panel),
        None
    );
    press.cancel();
    assert_eq!(
        press.up(PointerId::Mouse, Pos2::new(100.0, 100.0), panel),
        None
    );

    let snapshot = snapshot(vec![window(2, Rect::new(100, 100, 900, 700))]);
    let mut mode = be_win32::FocusMode::new();
    mode.set_hotkey(true);
    let since = mode.returns();
    mode.start(&snapshot, EntryId(2), None, since).unwrap();
    mode.step(&snapshot, be_win32::Foreground::Entry(EntryId(2)), &[]);
    mode.release(be_win32::EndReason::Hotkey);
    mode.observe(be_win32::Foreground::Own);
    assert_eq!(shell_focus(window_id(EntryId(2))), None);
    mode.step(&snapshot, be_win32::Foreground::Own, &[]);
    assert_eq!(mode.focused(), None);
}
