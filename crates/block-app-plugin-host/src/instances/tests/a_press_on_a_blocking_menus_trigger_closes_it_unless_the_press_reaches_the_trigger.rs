use super::*;

#[test]
fn a_press_on_a_blocking_menus_trigger_closes_it_unless_the_press_reaches_the_trigger() {
    let mut instances = layered(pos2(10.0, 10.0));
    let menu = Rect::from_min_size(pos2(50.0, 50.0), vec2(60.0, 80.0));
    let button = Rect::from_min_size(pos2(50.0, 20.0), vec2(30.0, 20.0));
    let mut opened = layer(1, 0, menu);
    opened.placement.trigger = Some(button);
    opened.placement.blocks_owner = true;
    report_anchored(&mut instances, vec![opened]);
    let shown = instances.shown_anchored(INSTANCE, REGION);
    let on_button = shown[0]
        .trigger
        .expect("the trigger reaches the host")
        .center();
    assert_eq!(
        crate::dismissed_by_press(&shown, on_button),
        vec![ViewId(1)],
        "a menu that blocks its view keeps the press from its button, so the press closes it"
    );

    let mut instances = layered(pos2(10.0, 10.0));
    let bar = Rect::from_min_size(pos2(40.0, 10.0), vec2(120.0, 40.0));
    let mut tools = layer(1, 0, bar);
    tools.placement.dismiss_on_outside_press = false;
    let mut opened = layer(2, 1, menu);
    opened.placement.trigger = Some(button);
    opened.placement.blocks_owner = true;
    report_anchored(&mut instances, vec![tools, opened]);
    let shown = instances.shown_anchored(INSTANCE, REGION);
    let on_button = shown[1]
        .trigger
        .expect("the trigger reaches the host")
        .center();
    assert!(
        shown[0].takes(on_button),
        "the button sits in the bar's own view"
    );
    assert!(
        crate::dismissed_by_press(&shown, on_button).is_empty(),
        "a press on a button in another view of the plugin reaches it, so the menu is left to the \
         button, whose click closes it, rather than closed for the click to open it again"
    );
}
