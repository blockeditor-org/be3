use super::*;

/// A layer of few edits but more bytes than a peer shows is not previewed there until its
/// holder's next layer.
#[test]
fn a_peer_layer_too_many_bytes_to_show_is_not_previewed_until_its_next() {
    let (mut sim, a, b) = super::tentative::dragging(2);
    let long = "x".repeat(crate::replica::LARGEST_RELAYED_OPERATION / 2 + 1);
    let large = [Checklist::set_text(a, &long), Checklist::set_text(b, &long)];
    super::tentative::relay_held(&mut sim, 1, 0, 5, &large, None);
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "b"]);
    super::tentative::relay_held(&mut sim, 1, 0, 6, &[Checklist::set_text(b, "next")], None);
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "next"]);
}
