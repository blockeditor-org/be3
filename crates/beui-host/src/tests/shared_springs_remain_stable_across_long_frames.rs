use crate::reactive::{Easing, Spring, SpringMotion, tween};

/// A spring moves the same whatever the frame rate, passing its rest within a step included:
/// one long step lands where many short ones do.
#[test]
fn shared_springs_remain_stable_across_long_frames() {
    for (spring, from, speed) in [
        (
            Spring {
                stiffness: 180.0,
                damping: 26.832_815,
            },
            100.0,
            -40.0,
        ),
        (
            Spring {
                stiffness: 400.0,
                damping: 50.0,
            },
            100.0,
            -40.0,
        ),
        (
            Spring {
                stiffness: 500.0,
                damping: 33.541_02,
            },
            100.0,
            -40.0,
        ),
        // Passes its rest within the step.
        (
            Spring {
                stiffness: 500.0,
                damping: 33.541_02,
            },
            10.0,
            -400.0,
        ),
    ] {
        let mut one = SpringMotion::new(from, speed);
        one.advance(spring, 0.1);
        let mut many = SpringMotion::new(from, speed);
        for _ in 0..10 {
            many.advance(spring, 0.01);
        }
        assert!(
            (one.displacement - many.displacement).abs() < 0.001
                && (one.velocity - many.velocity).abs() < 0.001,
            "{spring:?} from {from} at {speed}: {one:?} in one step, {many:?} in ten"
        );
        one.advance(spring, 60.0);
        assert!(!one.moving());
    }
    assert_eq!(tween(0.0, 0.0, Easing::Linear), 1.0);
}
