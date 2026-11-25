use crate::reactive::{Easing, Spring, tween};

#[test]
fn shared_springs_remain_stable_across_long_frames() {
    for spring in [
        Spring {
            stiffness: 180.0,
            damping: 26.832_815,
        },
        Spring {
            stiffness: 400.0,
            damping: 40.0,
        },
        Spring {
            stiffness: 400.0,
            damping: 50.0,
        },
    ] {
        let (mut one, mut velocity, mut passed) = (100.0, -40.0, false);
        spring.advance(&mut one, &mut velocity, &mut passed, 0.1);
        let (mut many, mut speed, mut past) = (100.0, -40.0, false);
        for _ in 0..10 {
            spring.advance(&mut many, &mut speed, &mut past, 0.01);
        }
        assert!((one - many).abs() < 0.001);
        assert!((velocity - speed).abs() < 0.001);
        spring.advance(&mut one, &mut velocity, &mut passed, 60.0);
        assert_eq!((one, velocity), (0.0, 0.0));
    }
    assert_eq!(tween(0.0, 0.0, Easing::Linear), 1.0);
}
