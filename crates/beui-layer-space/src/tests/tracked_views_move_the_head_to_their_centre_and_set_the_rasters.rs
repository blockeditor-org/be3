use super::*;
use crate::head::{Fov, View, Views};

#[test]
fn tracked_views_move_the_head_to_their_centre_and_set_the_rasters() {
    let (mut space, now) = space();
    assert_eq!(
        space.views(),
        [space.head.view(PLATFORM)],
        "the simulator has one view, the head's, over the whole platform window"
    );
    let simulated = space
        .scale(WindowId::MAIN)
        .expect("the main panel has a scale");

    let turned = glam::Quat::from_rotation_y(0.1) * glam::Quat::from_rotation_x(-0.05);
    let centre = Head::default().position + Vec3::new(0.02, 0.01, 0.0);
    let eye = |side: f32, fov: Fov| View {
        position: centre + turned * Vec3::new(side * 0.032, 0.0, 0.0),
        rotation: turned,
        fov,
        size: PLATFORM * 2.0,
    };
    let views = Views {
        display_time: later(now, 0.011),
        views: vec![
            eye(-1.0, Fov::from_angles(-0.9, 0.7, 0.8, -0.8)),
            eye(1.0, Fov::from_angles(-0.7, 0.9, 0.8, -0.8)),
        ],
    };
    space.set_views(Some(views.clone()));
    assert!(
        close(space.head.position, centre),
        "the head is midway between the eyes: {}",
        space.head.position
    );
    assert!(
        space.head.rotation().angle_between(turned) < 1e-4,
        "and turned as they are"
    );
    assert_eq!(space.views(), views.views);
    assert_eq!(space.tracked(), Some(&views));

    send(&mut space, now, []);
    send(&mut space, later(now, 1.0), []);
    let tracked = space
        .scale(WindowId::MAIN)
        .expect("the main panel has a scale");
    assert!(
        tracked > simulated,
        "the headset's views have more pixels across the panel, so it is rastered sharper: \
         {simulated} then {tracked}"
    );

    space.set_views(None);
    assert_eq!(
        space.views(),
        [space.head.view(PLATFORM)],
        "without tracked views the simulator's head is the view again, where the eyes left it"
    );
}
