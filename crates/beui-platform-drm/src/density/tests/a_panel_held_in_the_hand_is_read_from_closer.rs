use super::*;

#[test]
fn a_panel_held_in_the_hand_is_read_from_closer() {
    let pinenote = natural_scale((1872, 1404), Some((210, 157)), true);
    let pinephone = natural_scale((720, 1440), Some((68, 136)), true);
    assert_eq!(pinenote, 1.5, "the PineNote's 10.3 inch e-paper panel");
    assert_eq!(pinephone, 1.75, "a 6 inch phone panel");
    assert_eq!(
        natural_scale((1872, 1404), Some((210, 157)), false),
        2.0,
        "the same panel on a desk would be read from further away"
    );
}
