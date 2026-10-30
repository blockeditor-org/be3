use super::*;

#[test]
fn a_press_outside_a_dismissable_chain_closes_it_and_what_hangs_from_it() {
    let mut instances = layered(pos2(10.0, 10.0));
    let menu = Rect::from_min_size(pos2(50.0, 50.0), vec2(60.0, 80.0));
    let submenu = Rect::from_min_size(pos2(110.0, 60.0), vec2(60.0, 40.0));
    let tooltip = Rect::from_min_size(pos2(300.0, 200.0), vec2(40.0, 20.0));
    let mut kept = layer(3, 0, tooltip);
    kept.placement.dismiss_on_outside_press = false;
    report_anchored(
        &mut instances,
        vec![layer(1, 0, menu), layer(2, 1, submenu), kept],
    );
    let shown = instances.shown_anchored(INSTANCE, REGION);

    assert!(
        crate::dismissed_by_press(&shown, pos2(120.0, 70.0)).is_empty(),
        "a press on a submenu is inside its menu's chain"
    );
    assert_eq!(
        crate::dismissed_by_press(&shown, pos2(60.0, 120.0)),
        vec![ViewId(2)],
        "a press on the menu closes only the submenu hanging from it"
    );
    let outside = crate::dismissed_by_press(&shown, pos2(5.0, 5.0));
    assert_eq!(
        outside,
        vec![ViewId(1), ViewId(2)],
        "a press outside the chain closes all of it, and leaves a layer that is not dismissable"
    );

    let messages = instances.dismiss_anchored_views(INSTANCE, REGION, outside);
    let batches: Vec<_> = messages
        .into_iter()
        .filter_map(|message| match message {
            Outbound::Plugin(PluginCall::Input(batch)) => Some(batch),
            _ => None,
        })
        .collect();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].dismissed, vec![ViewId(1), ViewId(2)]);
    assert!(
        batches[0].events.is_empty(),
        "the press itself is not taken: it still reaches what it landed on"
    );
}
