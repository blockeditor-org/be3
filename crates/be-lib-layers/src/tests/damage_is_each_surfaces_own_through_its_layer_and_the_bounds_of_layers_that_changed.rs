use super::*;

#[test]
fn damage_is_each_surfaces_own_through_its_layer_and_the_bounds_of_layers_that_changed() {
    let before = window(vec![
        layer(1, pos2(0.0, 0.0), (200, 200)),
        layer(2, pos2(10.0, 10.0), (20, 20)),
        layer(3, pos2(50.0, 50.0), (20, 20)),
    ]);
    let mut after = before.clone();
    after.layers[1].transform.x = 20.0;
    after.layers.remove(2);
    after.layers.push(layer(4, pos2(80.0, 80.0), (20, 20)));
    let damage = before.damage(&after, |layer| {
        (layer.id == LayerId(1)).then(|| vec![rect(100.0, 100.0, 300.0, 4.0)])
    });
    assert_eq!(
        damage,
        vec![
            rect(10.0, 10.0, 10.0, 10.0),
            rect(20.0, 10.0, 10.0, 10.0),
            rect(80.0, 80.0, 10.0, 10.0),
            rect(50.0, 50.0, 10.0, 10.0),
            rect(50.0, 50.0, 50.0, 2.0),
        ],
        "the moved layer's old and new bounds, the new layer, the gone one, then the root's own \
         damage clipped to what it shows"
    );
}
