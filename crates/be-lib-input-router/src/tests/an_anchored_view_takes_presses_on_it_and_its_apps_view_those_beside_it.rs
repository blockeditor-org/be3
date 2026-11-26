use super::*;

/// An anchored view (a menu an app opens) lies over the app's view: a press on it is its own,
/// a press beside it the view's, and neither's press holds the other.
#[test]
fn an_anchored_view_takes_presses_on_it_and_its_apps_view_those_beside_it() {
    const MENU: u32 = 3;
    let test = canvas_with_editor().with(MENU, rect(300.0, 300.0, 200.0, 200.0), Some(EDITOR));
    let mut router = Router::new(ROOT);
    let on_menu = router.route(&test, press(350.0, 350.0, true));
    assert_eq!(
        inputs_for(&on_menu, MENU).last(),
        Some(&press(50.0, 50.0, true)),
        "the menu hears the press in its own coordinates"
    );
    router.route(&test, press(350.0, 350.0, false));
    let beside = router.route(&test, press(250.0, 250.0, true));
    assert_eq!(
        inputs_for(&beside, EDITOR).last(),
        Some(&press(50.0, 50.0, true))
    );
    assert!(inputs_for(&beside, MENU).contains(&Input::PointerLeft));
}
