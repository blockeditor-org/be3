use beui_core::runner::{Controller, Hand, HandPose};

use super::*;
use crate::reactive::*;

#[test]
fn a_driver_aims_and_pulls_a_controller_through_the_host() {
    let _root = RootScope::new();
    let document = build_node(move || {
        view! {
            <Frame width=300.0 height=300.0 />
        }
    });
    let mut driven = crate::testing::Stepped::hosting_windows(
        context(),
        crate::testing::Showing(document),
        crate::WindowSize {
            size: VIEWPORT,
            scale: 1.0,
        },
    );
    driven
        .ask(&["xr", "right", "point", "120,80"])
        .expect("the laser points");
    driven
        .ask(&["xr", "left", "pose", "0.1", "1.2", "-0.3", "90"])
        .expect("the hand is posed");
    driven
        .ask(&["xr", "left", "squeeze", "0.8"])
        .expect("the grip is squeezed");
    driven
        .ask(&["xr", "right", "lost"])
        .expect("the controller is lost");
    let half = std::f32::consts::FRAC_1_SQRT_2;
    let posed = HandPose {
        position: [0.1, 1.2, -0.3],
        orientation: [0.0, half, 0.0, half],
    };
    let controllers = driven.windows().controllers.clone();
    assert_eq!(controllers.len(), 4, "{controllers:?}");
    assert_eq!(
        controllers[0],
        Controller::Toward {
            hand: Hand::Right,
            window: crate::WindowId::MAIN,
            at: pos2(120.0, 80.0),
        }
    );
    let Controller::Pose { hand, aim, grip } = controllers[1] else {
        panic!("{:?} is no pose", controllers[1]);
    };
    assert_eq!(
        (hand, aim.position, grip),
        (Hand::Left, posed.position, aim)
    );
    assert!(
        aim.orientation
            .iter()
            .zip(posed.orientation)
            .all(|(got, wanted)| (got - wanted).abs() < 1e-6),
        "a yaw of 90 turns the laser to the left: {aim:?}"
    );
    assert_eq!(
        controllers[2..],
        [
            Controller::Squeeze {
                hand: Hand::Left,
                value: 0.8,
            },
            Controller::Lost { hand: Hand::Right },
        ]
    );
    assert!(driven.ask(&["xr", "right", "trigger", "2"]).is_err());
    assert!(driven.ask(&["xr", "middle", "trigger", "1"]).is_err());
}
