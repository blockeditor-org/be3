use super::*;

#[test]
fn a_size_under_the_windows_least_size_is_not_a_pick() {
    let firefox = KnownWindow {
        size: Vec2::new(148.0, 200.0),
        min_size: Vec2::new(500.0, 120.0),
        maximized: true,
        ..KnownWindow::default()
    };
    let listed = firefox.listed();
    assert_eq!(
        listed.size, None,
        "GTK's first commit for Firefox is under Firefox's own least size"
    );
    assert!(listed.maximized);

    let foot = KnownWindow {
        size: Vec2::new(640.0, 400.0),
        min_size: Vec2::new(40.0, 20.0),
        ..KnownWindow::default()
    };
    assert_eq!(foot.listed().size, Some(Vec2::new(640.0, 400.0)));
}
