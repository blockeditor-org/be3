use super::*;

#[test]
fn a_press_on_an_anchored_views_trigger_leaves_it_to_the_trigger() {
    let mut instances = layered(pos2(10.0, 10.0));
    let menu = Rect::from_min_size(pos2(50.0, 50.0), vec2(60.0, 80.0));
    let button = Rect::from_min_size(pos2(50.0, 20.0), vec2(30.0, 20.0));
    let mut opened = layer(1, 0, menu);
    opened.placement.trigger = Some(button);
    report_anchored(&mut instances, vec![opened]);
    let shown = instances.shown_anchored(INSTANCE, REGION);
    let on_button = shown[0]
        .trigger
        .expect("the trigger reaches the host")
        .center();

    assert!(
        crate::dismissed_by_press(&shown, on_button).is_empty(),
        "a press on the button that opened the menu leaves the menu to the button, \
         whose click closes it, rather than closing it for the click to open it again"
    );
    assert_eq!(
        crate::dismissed_by_press(&shown, pos2(5.0, 5.0)),
        vec![ViewId(1)],
        "a press anywhere else still closes it"
    );
}
