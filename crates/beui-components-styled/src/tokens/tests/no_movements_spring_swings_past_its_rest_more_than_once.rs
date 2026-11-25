use super::*;

/// No motion rings: let go at rest, moving away from where it rests, or thrown hard towards
/// it, a movement's spring passes its rest at most once and lands there on the way back.
/// `Settle` and `Pull` overshoot once, visibly, from rest; `Rubber` never does.
#[test]
fn no_movements_spring_swings_past_its_rest_more_than_once() {
    for movement in Movement::ALL {
        let spring = movement.spring();
        let mut overshoots_from_rest = false;
        for (from, speed) in [
            (600.0_f32, 0.0_f32),
            (100.0, 0.0),
            (100.0, 800.0),
            (-40.0, -300.0),
            (300.0, -6000.0),
            (2000.0, 0.0),
        ] {
            let (mut at, mut velocity, mut passed) = (from, speed, false);
            let mut crossings = 0;
            let mut deepest = 0.0_f32;
            for frame in 0..600 {
                let was = at;
                spring.advance(&mut at, &mut velocity, &mut passed, 1.0 / 60.0);
                if at * was < 0.0 {
                    crossings += 1;
                }
                if at * from < 0.0 {
                    deepest = deepest.max(at.abs());
                }
                assert!(
                    crossings <= 1,
                    "{} from {from} at {speed} passed its rest again, to {at} in frame {frame}",
                    movement.name()
                );
            }
            assert_eq!(
                (at, velocity),
                (0.0, 0.0),
                "{} from {from} at {speed} comes to rest",
                movement.name()
            );
            if from == 600.0 && deepest > 4.0 {
                overshoots_from_rest = true;
            }
        }
        assert_eq!(
            overshoots_from_rest,
            movement != Movement::Rubber,
            "{}: only content pulled past its end never overshoots",
            movement.name()
        );
    }
}
