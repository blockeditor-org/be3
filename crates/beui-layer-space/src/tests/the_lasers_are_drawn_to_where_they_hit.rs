use super::the_chrome_around_a_curved_panel::window;
use super::*;

#[test]
fn the_lasers_are_drawn_to_where_they_hit() {
    let (mut space, now, context) = painted_space();
    let popped = open(
        &mut space,
        now,
        WindowPlacement::Over {
            window: WindowId::MAIN,
            at: pos2(330.0, 170.0),
        },
    )
    .0;
    let main = *main(&space);
    let target = space.panel(popped).unwrap().world(pos2(60.0, 90.0));
    drive(
        &mut space,
        now,
        [
            hand_pose_of(Hand::Left, left_hand(), main.world(pos2(260.0, 380.0))),
            hand_pose_of(Hand::Right, right_hand(), target),
            trigger(Hand::Right, 1.0),
        ],
    );
    let beams = space.beams();
    assert_eq!(beams.len(), 2);
    assert!(beams.iter().all(|beam| beam.hit));
    assert!(beams[1].end.distance(target) < 1e-3);
    let windows = [
        window(&context, WindowId::MAIN, PLATFORM, "Main"),
        window(&context, popped, SMALL, "Popped out"),
    ];
    assert_scene(
        &mut space,
        now,
        &windows,
        "the_lasers_are_drawn_to_where_they_hit",
    );

    drive(&mut space, now, [Controller::Lost { hand: Hand::Left }]);
    assert_eq!(
        space.beams().len(),
        1,
        "a controller lost from tracking draws no laser"
    );
}
