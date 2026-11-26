use super::*;

/// A layer with more edits than a peer shows is not previewed there, nor any more of it, until
/// its holder's next layer.
#[test]
fn a_peer_layer_too_large_to_show_is_not_previewed_until_its_next() {
    let (mut sim, a, b) = super::tentative::dragging(2);
    let many: Vec<Edit> = (0..300)
        .map(|at| Checklist::set_text(a, &format!("{at}")))
        .collect();
    super::tentative::relay_held(&mut sim, 1, 0, 5, &many, None);
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "b"]);
    super::tentative::relay_held(&mut sim, 1, 0, 5, &[Checklist::set_text(a, "same")], None);
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "b"], "nor any more of it");
    super::tentative::relay_held(&mut sim, 1, 0, 6, &[Checklist::set_text(b, "next")], None);
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "next"]);
}
