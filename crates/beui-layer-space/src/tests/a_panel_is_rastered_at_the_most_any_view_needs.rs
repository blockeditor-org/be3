use super::*;
use crate::head::View;
use crate::scale::wanted;

#[test]
fn a_panel_is_rastered_at_the_most_any_view_needs() {
    let head = Head::default();
    let shown = panel(WindowId::MAIN, ahead(1.5));
    let eye = head.view(PLATFORM);
    let sharp = View {
        size: PLATFORM * 2.0,
        ..eye
    };
    let near = View {
        position: eye.position + Vec3::new(0.0, 0.0, -0.75),
        ..eye
    };
    let away = View {
        rotation: glam::Quat::from_rotation_y(std::f32::consts::PI),
        ..eye
    };
    let one = wanted(&shown, &[eye]).expect("the panel is in view");
    let twice = wanted(&shown, &[sharp]).expect("the panel is in view");
    assert!(
        (twice / one - 2.0).abs() < 1e-3,
        "a view with twice the pixels across the same field wants twice the raster: {one} and {twice}"
    );
    let nearer = wanted(&shown, &[near]).expect("the panel is in view");
    assert!(
        (nearer / one - 2.0).abs() < 1e-3,
        "a view half as far away wants twice the raster: {one} and {nearer}"
    );
    assert_eq!(
        wanted(&shown, &[eye, sharp, near]),
        Some(twice.max(nearer)),
        "the panel takes the most any view needs"
    );
    assert_eq!(
        wanted(&shown, &[away]),
        None,
        "a view facing away needs nothing"
    );
    assert_eq!(
        wanted(&shown, &[away, eye]),
        Some(one),
        "a view that cannot see the panel leaves it to the ones that can"
    );
}
