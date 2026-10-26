use super::*;
use crate::panel::pick;

#[test]
fn a_ray_hits_the_nearest_panel_where_it_points() {
    let near = WindowId::next();
    let far = WindowId::next();
    let panels = [panel(far, ahead(3.0)), panel(near, ahead(2.0))];
    let head = Head::default();

    let hit = pick(&panels, head.ray(PLATFORM, PLATFORM / 2.0)).expect("the ray hits");
    assert_eq!(hit.window, near, "the nearer panel is hit");
    assert!((hit.distance - 2.0).abs() < 1e-4);
    assert!(
        (hit.pos - pos2(500.0, 250.0)).length() < 0.01,
        "straight ahead is the middle of the window, at {:?}",
        hit.pos
    );

    let ray = head.ray(PLATFORM, vec2(PLATFORM.x / 2.0, PLATFORM.y / 2.0 - 50.0));
    let up = pick(&panels, ray).expect("a little above the middle still hits");
    assert!(up.pos.y < 250.0, "a ray above the middle hits higher up");

    let mut turned = head;
    turned.yaw = std::f32::consts::PI;
    assert_eq!(
        pick(&panels, turned.ray(PLATFORM, PLATFORM / 2.0)),
        None,
        "behind the viewer there is nothing"
    );

    let behind = Head {
        position: Vec3::new(0.0, crate::head::EYE_HEIGHT, -5.0),
        ..head
    };
    let mut back = behind;
    back.yaw = std::f32::consts::PI;
    assert_eq!(
        pick(&panels, back.ray(PLATFORM, PLATFORM / 2.0)),
        None,
        "a panel's back takes no pointer"
    );

    let wide = head.ray(PLATFORM, vec2(0.0, PLATFORM.y / 2.0));
    assert_eq!(
        pick(&panels, wide),
        None,
        "a ray past the panel's edge misses it"
    );
}
