use super::*;

#[test]
fn a_window_opened_at_a_host_placement_lands_on_that_pose() {
    let (mut space, now) = space();
    let mut shaped = panel(
        WindowId::next(),
        Pose {
            position: Vec3::new(0.4, 1.2, -1.5),
            rotation: glam::Quat::from_rotation_y(0.3) * glam::Quat::from_rotation_z(0.2),
        },
    );
    shaped.metres_per_point = 0.0017;
    shaped.bend = 0.8;
    let placement = shaped.placement();
    let mut read = panel(shaped.window, ahead(1.0));
    assert!(read.placed(&placement));
    assert_eq!(
        read, shaped,
        "a pose, scale and bend survive being written as a placement"
    );

    let (window, sent) = open(&mut space, now, WindowPlacement::Host(placement.clone()));
    let panel = *space.panel(window).expect("the window has a panel");
    assert!(close(panel.pose.position, shaped.pose.position));
    assert!(panel.pose.rotation.angle_between(shaped.pose.rotation) < 1e-4);
    assert_eq!(
        (panel.metres_per_point, panel.bend),
        (shaped.metres_per_point, shaped.bend),
        "the window opens at the scale and bend it was placed with"
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
                placement: placement.clone()
            },
        ],
        "the placement it reports opens it in the same place again"
    );

    for foreign in [
        "desktop 10 20",
        "space 0 1 -1 0 0 0 1",
        "space 0 1 -1 0 0 0 1 0 0",
        "space 0 1 -1 0 0 0 1 0.001 NaN",
    ] {
        let (other, _) = open(
            &mut space,
            now,
            WindowPlacement::Host(HostPlacement(foreign.to_owned())),
        );
        let opened = space.panel(other).expect("the window still opens");
        assert_eq!(
            (opened.metres_per_point, opened.bend),
            (1.0 / crate::panel::POINTS_PER_METRE, 0.0),
            "{foreign:?} is not a placement this host wrote, so it opens at the pointer"
        );
    }
}
