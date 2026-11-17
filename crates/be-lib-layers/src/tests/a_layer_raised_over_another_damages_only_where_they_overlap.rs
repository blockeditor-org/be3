use super::*;

#[test]
fn a_layer_raised_over_another_damages_only_where_they_overlap() {
    let before = window(vec![
        layer(1, pos2(0.0, 0.0), (20, 20)),
        layer(2, pos2(5.0, 5.0), (20, 20)),
    ]);
    let after = window(vec![before.layers[1].clone(), before.layers[0].clone()]);
    let damage = before.damage(&after, |_| Some(Vec::new()));
    assert_eq!(
        damage,
        vec![rect(5.0, 5.0, 5.0, 5.0)],
        "only where the two overlap shows differently"
    );
    assert!(
        before.damage(&before, |_| Some(Vec::new())).is_empty(),
        "nothing changed, nothing is damaged"
    );
}
