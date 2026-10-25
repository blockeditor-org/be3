use super::*;
use crate::{OPEN_DISTANCE, OPEN_GRAB, OPEN_NEARER};

#[test]
fn a_window_opened_at_the_pointer_lands_on_the_cursor_ray_facing_the_viewer() {
    let (mut space, now) = space();
    let at = centre() + vec2(150.0, -80.0);
    send(&mut space, now, [Input::PointerMoved(at)]);
    let ray = space.head.ray(PLATFORM, at.to_vec2());
    let under = main(&space)
        .hit(ray)
        .expect("the cursor is on the main window");

    let (window, sent) = open(&mut space, now, WindowPlacement::AtPointer);
    let panel = space.panel(window).expect("the window has a panel").clone();
    assert_eq!(panel.size, SMALL, "at the size the app asked for");
    let grabbed = panel.world(OPEN_GRAB);
    assert!(
        close(grabbed, ray.at(under.distance - OPEN_NEARER)),
        "its corner is on the cursor's ray, just in front of what the ray hit"
    );
    let normal = panel.normal();
    assert!(level(&panel) < 1e-4, "its edges are level: {normal:?}");
    let toward = (space.head.position - panel.world(OPEN_GRAB)).normalize();
    assert!(
        toward.y.abs() > 0.05 && close(normal, toward),
        "it faces the viewer, tilted up toward the eye: {normal:?}"
    );
    assert_eq!(
        reports(&sent),
        [
            WindowReport::Resized {
                window,
                size: SMALL
            },
            WindowReport::Placed {
                window,
                placement: panel.placement()
            },
        ],
        "the host reports where it put the window"
    );
    let over = delivered(&send(&mut space, now, [Input::PointerMoved(at)]));
    let [(to, moved)] = over.as_slice() else {
        panic!("one move, not {over:?}");
    };
    assert_eq!(*to, window, "the pointer is now over the new window");
    assert!((pos_of(moved) - OPEN_GRAB).length() < 0.01, "{moved:?}");

    let corner = pos2(2.0, 2.0);
    send(&mut space, now, [Input::PointerMoved(corner)]);
    let (far, _) = open(&mut space, now, WindowPlacement::AtPointer);
    let ray = space.head.ray(PLATFORM, corner.to_vec2());
    assert!(
        close(
            space.panel(far).unwrap().world(OPEN_GRAB),
            ray.at(OPEN_DISTANCE)
        ),
        "where the ray hits nothing it opens at a default distance"
    );
}
