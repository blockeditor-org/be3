use super::*;

/// No motion oscillates: every movement's spring is critically damped or overdamped, so let go
/// at rest, or moving away from where it rests, it comes to rest without passing it.
#[test]
fn no_movements_spring_passes_where_it_comes_to_rest() {
    for movement in Movement::ALL {
        let spring = movement.spring();
        assert!(
            spring.never_oscillates(),
            "{} is damped below critical: {spring:?}",
            movement.name()
        );
        for (from, speed) in [(100.0_f32, 0.0_f32), (100.0, 800.0), (-40.0, -300.0)] {
            let (mut at, mut velocity) = (from, speed);
            for frame in 0..600 {
                spring.advance(&mut at, &mut velocity, 1.0 / 60.0);
                assert!(
                    at * from >= 0.0,
                    "{} from {from} at {speed} passed its rest, to {at} in frame {frame}",
                    movement.name()
                );
            }
            assert_eq!(
                (at, velocity),
                (0.0, 0.0),
                "{} from {from} at {speed} comes to rest",
                movement.name()
            );
        }
    }
}
