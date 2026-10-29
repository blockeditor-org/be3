use super::*;

#[test]
fn a_child_placed_in_a_layer_names_it() {
    let mut rig = Rig::new(Some(space()));
    let popover = Rect::from_min_size(pos2(220.0, 130.0), vec2(100.0, 80.0));
    rig.open(vec![layer(3, LayerParent::Region, popover)]);
    *rig.shared.child_in.borrow_mut() = Some(LayerId(3));
    let commit = rig.draw();
    let placed: Vec<_> = commit
        .children
        .iter()
        .flat_map(|placements| &placements.children)
        .map(|child| (child.in_layer, child.rect))
        .collect();
    assert_eq!(
        placed,
        [(
            Some(LayerId(3)),
            Rect::from_min_size(pos2(40.0, 50.0), vec2(30.0, 20.0))
        )],
        "the child is committed in the layer it was placed in, at its rect in host coordinates"
    );
    assert_eq!(Rig::layers(&commit).len(), 1, "with the layer holding it");
}
