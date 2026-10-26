use super::*;
use crate::head::Fov;

#[test]
fn an_off_axis_view_projects_its_field_of_view_tangents_to_the_edges_of_its_target() {
    let (left, right, up, down) = (-1.2_f32, 0.5_f32, 0.8_f32, -0.3_f32);
    let fov = Fov::tangents(left, right, up, down);
    let projected = |x: f32, y: f32, depth: f32| {
        let clip = fov.projection() * glam::Vec4::new(x * depth, y * depth, -depth, 1.0);
        clip.truncate() / clip.w
    };
    for depth in [0.5, 2.0, 30.0] {
        let corners = [
            (left, up, glam::Vec2::new(-1.0, 1.0)),
            (right, up, glam::Vec2::new(1.0, 1.0)),
            (left, down, glam::Vec2::new(-1.0, -1.0)),
            (right, down, glam::Vec2::new(1.0, -1.0)),
        ];
        for (x, y, edge) in corners {
            let ndc = projected(x, y, depth);
            assert!(
                (ndc.truncate() - edge).length() < 1e-5,
                "the tangents ({x}, {y}) at {depth} m land on the target's corner {edge}, not {ndc}"
            );
            assert!(
                (0.0..=1.0).contains(&ndc.z),
                "a point in front of the eye has a depth in range: {}",
                ndc.z
            );
        }
    }
    assert!(
        projected(0.0, 0.0, 1.0).x > 0.0,
        "straight ahead is right of the middle when the view reaches further left"
    );

    let angles = Fov::from_angles(left.atan(), right.atan(), up.atan(), down.atan());
    assert!(
        angles.projection().abs_diff_eq(fov.projection(), 1e-5),
        "angles are the tangents' arctangents, as OpenXR gives them"
    );

    let symmetric = Fov::symmetric(PLATFORM);
    let half = (crate::head::FOV_Y / 2.0).tan();
    let aspect = PLATFORM.x / PLATFORM.y;
    let general = Fov::tangents(-half * aspect, half * aspect, half, -half);
    assert!(
        symmetric
            .projection()
            .abs_diff_eq(general.projection(), 1e-5),
        "the simulator's view is the symmetric case of the same projection"
    );
}
