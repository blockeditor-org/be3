use super::*;
use block_plugin_api::LayerParent;

#[test]
fn a_child_placed_in_a_layer_is_shown_in_host_coordinates_inside_it() {
    let mut instances = layered(pos2(10.0, 10.0));
    let card = Rect::from_min_size(pos2(200.0, 100.0), vec2(120.0, 90.0));
    report_layers(&mut instances, vec![layer(1, LayerParent::Region, card)]);
    let picker = Rect::from_min_size(pos2(210.0, 110.0), vec2(200.0, 60.0));
    let mut placements = child(picker, false);
    placements.children[0].in_layer = Some(LayerId(1));
    placements.children[0].mode = ChildMode::Active;
    placements.children[0].clip = picker;
    let mut stray = placements.children[0].clone();
    stray.child = ChildId(2);
    stray.in_layer = Some(LayerId(9));
    placements.children.push(stray);
    instances.set_children(placements);

    let region = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    let (children, holes) = instances.host_children(INSTANCE, REGION, region, region, None);

    assert_eq!(
        children.iter().map(|child| child.child).collect::<Vec<_>>(),
        vec![ChildId(1)],
        "a child naming a layer the region does not have open is left out"
    );
    let shown = &children[0];
    assert_eq!(shown.in_layer, Some(LayerId(1)));
    assert_eq!(shown.rect, picker, "its rect is already in host coordinates");
    assert_eq!(
        shown.clip,
        picker.intersect(card),
        "it is clipped to its layer, not to its region"
    );
    assert!(
        holes.parts().is_empty(),
        "it opens no hole in the region, which it is not drawn in"
    );

    place_layered(&mut instances, pos2(30.0, 10.0), window());
    let (children, _) = instances.host_children(
        INSTANCE,
        REGION,
        Rect::from_min_size(pos2(30.0, 10.0), SIZE),
        region,
        None,
    );
    assert_eq!(
        children[0].rect,
        picker.translate(vec2(20.0, 0.0)),
        "it moves with its layer when the region moves"
    );
}
