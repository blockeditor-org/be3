use super::*;

#[test]
fn a_region_without_a_layer_space_opens_no_layers() {
    let mut rig = Rig::new(None);
    rig.open(vec![layer(
        1,
        LayerParent::Region,
        Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 10.0)),
    )]);
    let commit = rig.draw();
    assert_eq!(rig.shared.seen.borrow().spaces, [None]);
    assert!(
        Rig::layers(&commit).is_empty(),
        "the layer stays out of the report"
    );
    assert!(rig.shared.seen.borrow().painted.is_empty());
    assert!(rig.surfaces.layer_surfaces().is_empty());
}
