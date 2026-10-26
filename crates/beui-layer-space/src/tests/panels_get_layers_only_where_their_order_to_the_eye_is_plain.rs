use super::*;
use crate::layers::{Allowance, Candidate, choose};

fn candidate(panel: Panel) -> Candidate {
    Candidate {
        panel,
        pixels: (1000, 500),
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        version: 1,
    }
}

fn turned(window: WindowId, distance: f32, yaw: f32) -> Panel {
    let mut panel = panel(window, ahead(distance));
    panel.pose.rotation = glam::Quat::from_rotation_y(yaw);
    panel
}

const ALLOWED: Allowance = Allowance {
    most: 8,
    cylinders: true,
    largest: (4096, 4096),
};

fn chosen(panels: &[Panel], allowance: &Allowance) -> Vec<WindowId> {
    let candidates: Vec<Candidate> = panels.iter().copied().map(candidate).collect();
    choose(&candidates, Head::default().position, allowance)
        .iter()
        .map(|layer| layer.window)
        .collect()
}

#[test]
fn panels_get_layers_only_where_their_order_to_the_eye_is_plain() {
    let (back, front, aside) = (WindowId::next(), WindowId::next(), WindowId::next());
    let mut beside = panel(aside, ahead(1.0));
    beside.pose.position.x += 3.0;
    assert_eq!(
        chosen(
            &[panel(front, ahead(0.9)), panel(back, ahead(1.0)), beside],
            &ALLOWED
        ),
        [aside, back, front],
        "a panel in front of another is layered over it, back to front, and one apart from \
         both has a layer of its own, farthest first"
    );

    let crossing = chosen(
        &[turned(back, 1.0, 0.6), turned(front, 1.0, -0.6)],
        &ALLOWED,
    );
    assert_eq!(
        crossing.len(),
        1,
        "of two panels through each other only one is layered, and the eye buffer draws \
         the other with depth: {crossing:?}"
    );

    let mut bent = panel(back, ahead(1.0));
    bent.bend = 1.0;
    assert_eq!(
        chosen(&[bent], &ALLOWED),
        [back],
        "a bent panel is a cylinder"
    );
    assert_eq!(
        chosen(
            &[bent],
            &Allowance {
                cylinders: false,
                ..ALLOWED
            }
        ),
        [],
        "but not where the runtime has no cylinders"
    );

    assert_eq!(
        chosen(&[turned(back, 1.0, std::f32::consts::PI)], &ALLOWED),
        [],
        "a panel seen from behind has no layer"
    );
    assert_eq!(
        chosen(
            &[panel(front, ahead(0.9)), panel(back, ahead(1.0))],
            &Allowance { most: 1, ..ALLOWED }
        ),
        [front],
        "the runtime's limit keeps the panels that look biggest"
    );
    assert_eq!(
        chosen(
            &[panel(back, ahead(1.0))],
            &Allowance {
                largest: (512, 512),
                ..ALLOWED
            }
        ),
        [],
        "nor one larger than the runtime's swapchains"
    );
}
