use super::*;

#[test]
fn a_press_outside_a_surface_that_captures_presses_goes_to_its_placer() {
    let mut test = canvas_with_editor().with(3, rect(800.0, 800.0, 100.0, 100.0), Some(ROOT));
    test.last().captures = true;
    test.last().focus = false;
    let mut router = Router::new(ROOT);
    let pressed = router.route(&test, press(250.0, 250.0, true));
    assert_eq!(
        targets(&pressed),
        vec![ROOT, ROOT],
        "the compositor that placed the popup hears the press, to dismiss it, and nothing under it"
    );
}
