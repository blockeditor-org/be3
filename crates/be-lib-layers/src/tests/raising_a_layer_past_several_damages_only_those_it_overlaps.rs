use super::*;

#[test]
fn raising_a_layer_past_several_damages_only_those_it_overlaps() {
    let before = window(vec![
        layer(1, pos2(0.0, 0.0), (40, 40)),
        layer(2, pos2(10.0, 10.0), (40, 40)),
        layer(3, pos2(100.0, 100.0), (20, 20)),
        layer(4, pos2(200.0, 0.0), (20, 20)),
    ]);
    let mut after = before.clone();
    let raised = after.layers.remove(0);
    after.layers.push(raised);
    let damage = before.damage(&after, |_| Some(Vec::new()));
    assert_eq!(
        damage,
        vec![rect(10.0, 10.0, 10.0, 10.0)],
        "the raised layer passes three, but overlaps only the second"
    );
}
