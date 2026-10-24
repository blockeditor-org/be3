use super::*;

#[test]
fn a_window_opened_at_a_host_placement_lands_on_that_pose() {
    let (mut space, now) = space();
    let pose = Pose {
        position: Vec3::new(0.4, 1.2, -1.5),
        rotation: glam::Quat::from_rotation_y(0.3),
    };
    let placement = pose.placement();
    assert_eq!(
        Pose::from_placement(&placement),
        Some(pose),
        "a pose survives being written as a placement"
    );

    let (window, sent) = open(&mut space, now, WindowPlacement::Host(placement.clone()));
    let panel = space.panel(window).expect("the window has a panel");
    assert!(close(panel.pose.position, pose.position));
    assert!(panel.pose.rotation.angle_between(pose.rotation) < 1e-4);
    assert_eq!(
        reports(&sent),
        [
            WindowReport::Resized {
                window,
                size: SMALL
            },
            WindowReport::Placed {
                window,
                placement: panel.pose.placement()
            },
        ]
    );
    assert_eq!(
        Pose::from_placement(&panel.pose.placement()),
        Some(panel.pose),
        "the placement it reports opens it in the same place again"
    );

    let (other, _) = open(
        &mut space,
        now,
        WindowPlacement::Host(HostPlacement("desktop 10 20".to_owned())),
    );
    assert!(
        space.panel(other).is_some(),
        "a placement another host wrote opens at the pointer instead"
    );
}
