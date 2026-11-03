use super::*;

fn banner(shown: &mut Shown) -> bool {
    shown.has("host.window.not_responding")
}

#[test]
fn a_window_that_is_not_responding_offers_to_wait_or_force_close() {
    let mut shown = shown(status(SIZE, Vec2::ZERO, true));
    assert!(!banner(&mut shown), "a responding window has no banner");

    shown.status.set(Some(status(SIZE, Vec2::ZERO, false)));
    settle(&mut shown.stepped);
    assert!(banner(&mut shown), "a window that stops responding says so");
    shown
        .stepped
        .snapshot("a_window_that_is_not_responding_offers_to_wait_or_force_close");

    shown.stepped.click("host.window.wait");
    settle(&mut shown.stepped);
    assert!(!banner(&mut shown), "waiting puts the banner away");

    shown.status.set(Some(status(SIZE, Vec2::ZERO, true)));
    settle(&mut shown.stepped);
    shown.status.set(Some(status(SIZE, Vec2::ZERO, false)));
    settle(&mut shown.stepped);
    assert!(
        banner(&mut shown),
        "a window that stops responding again says so again"
    );

    shown.stepped.click("host.window.force_close");
    settle(&mut shown.stepped);
    assert_eq!(shown.closed.get(), 1, "force close closes the window");
}
