use super::*;

#[test]
fn a_host_transform_maps_region_points_to_host_coordinates() {
    let to_host = HostTransform {
        origin: Pos2::new(100.0, 50.0),
        scale: Vec2::new(2.0, 0.5),
    };
    assert_eq!(to_host.point(Pos2::new(10.0, 40.0)), Pos2::new(120.0, 70.0));
    assert_eq!(
        to_host.inverse_point(Pos2::new(120.0, 70.0)),
        Pos2::new(10.0, 40.0)
    );
    assert_eq!(
        to_host.rect(Rect::from_min_size(Pos2::ZERO, Vec2::new(10.0, 10.0))),
        Rect::from_min_max(Pos2::new(100.0, 50.0), Pos2::new(120.0, 55.0))
    );
    assert_eq!(
        HostTransform::IDENTITY.point(Pos2::new(3.0, 4.0)),
        Pos2::new(3.0, 4.0)
    );
}
