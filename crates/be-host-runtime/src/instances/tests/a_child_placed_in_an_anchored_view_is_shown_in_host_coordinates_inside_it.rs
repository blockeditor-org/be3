use super::*;

#[test]
fn a_child_placed_in_an_anchored_view_is_shown_in_host_coordinates_inside_it() {
    let mut instances = layered(pos2(10.0, 10.0));
    let card = Rect::from_min_size(pos2(200.0, 100.0), vec2(120.0, 90.0));
    report_anchored(&mut instances, vec![layer(1, 0, card)]);
    let picker = Rect::from_min_size(pos2(210.0, 110.0), vec2(200.0, 60.0));
    let own = picker.translate(-card.min.to_vec2());
    let mut placements = child(own, false);
    placements.view = ViewId(1);
    placements.children[0].mode = ChildMode::Active;
    placements.children[0].clip = own;
    let mut stray = placements.clone();
    stray.view = ViewId(9);
    stray.children[0].child = ChildId(2);
    instances.set_children(placements);
    instances.set_children(stray);

    let region = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    let (children, holes) = instances.host_children(INSTANCE, REGION, region, region, None);

    assert_eq!(
        children.iter().map(|child| child.child).collect::<Vec<_>>(),
        vec![ChildId(1)],
        "a child of a view the region does not have open is left out"
    );
    let shown = &children[0];
    assert_eq!(shown.anchored, Some(ViewId(1)));
    assert_eq!(
        shown.rect, picker,
        "its rect in the anchored view's own points is shown in host coordinates"
    );
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
