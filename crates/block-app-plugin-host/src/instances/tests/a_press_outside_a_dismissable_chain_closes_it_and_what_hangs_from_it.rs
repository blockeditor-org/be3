use super::*;
use block_plugin_api::LayerParent;

#[test]
fn a_press_outside_a_dismissable_chain_closes_it_and_what_hangs_from_it() {
    let mut instances = layered(pos2(10.0, 10.0));
    let menu = Rect::from_min_size(pos2(50.0, 50.0), vec2(60.0, 80.0));
    let submenu = Rect::from_min_size(pos2(110.0, 60.0), vec2(60.0, 40.0));
    let tooltip = Rect::from_min_size(pos2(300.0, 200.0), vec2(40.0, 20.0));
    let mut kept = layer(3, LayerParent::Region, tooltip);
    kept.dismiss_on_outside_press = false;
    report_layers(
        &mut instances,
        vec![
            layer(1, LayerParent::Region, menu),
            layer(2, LayerParent::Layer(LayerId(1)), submenu),
            kept,
        ],
    );
    let shown = instances.layers(INSTANCE, REGION);

    assert!(
        crate::dismissed_by_press(&shown, pos2(120.0, 70.0)).is_empty(),
        "a press on a submenu is inside its menu's chain"
    );
    assert_eq!(
        crate::dismissed_by_press(&shown, pos2(60.0, 120.0)),
        vec![LayerId(2)],
        "a press on the menu closes only the submenu hanging from it"
    );
    let outside = crate::dismissed_by_press(&shown, pos2(5.0, 5.0));
    assert_eq!(
        outside,
        vec![LayerId(1), LayerId(2)],
        "a press outside the chain closes all of it, and leaves a layer that is not dismissable"
    );

    let messages = instances.dismiss_layers(INSTANCE, REGION, outside);
    let batches: Vec<_> = messages
        .into_iter()
        .filter_map(|message| match message {
            Outbound::Plugin(PluginCall::Input(batch)) => Some(batch),
            _ => None,
        })
        .collect();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].dismissed, vec![LayerId(1), LayerId(2)]);
    assert!(
        batches[0].events.is_empty(),
        "the press itself is not taken: it still reaches what it landed on"
    );
}
