use super::*;

#[test]
fn a_layer_raised_over_another_damages_both_of_its_places() {
    let before = window(vec![
        layer(1, pos2(0.0, 0.0), (20, 20)),
        layer(2, pos2(5.0, 5.0), (20, 20)),
    ]);
    let after = window(vec![before.layers[1].clone(), before.layers[0].clone()]);
    let damage = before.damage(&after, |_| Some(Vec::new()));
    assert!(
        damage.contains(&rect(0.0, 0.0, 10.0, 10.0))
            && damage.contains(&rect(5.0, 5.0, 10.0, 10.0)),
        "{damage:?}"
    );
    assert!(
        before.damage(&before, |_| Some(Vec::new())).is_empty(),
        "nothing changed, nothing is damaged"
    );
}
