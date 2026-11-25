use crate::styled::Movement;

#[test]
fn every_movement_comes_to_rest_where_it_is_going() {
    for movement in Movement::ALL {
        let easing = movement.easing();
        assert_eq!(
            easing.sample(0.0),
            0.0,
            "{} starts where it was",
            movement.name()
        );
        assert_eq!(
            easing.sample(1.0),
            1.0,
            "{} ends where it goes",
            movement.name()
        );

        // A spring rests by pixel distances, so it is let go a long way off.
        let spring = movement.spring();
        let (mut displacement, mut velocity, mut passed) = (1000.0_f32, 0.0_f32, false);
        let mut frames = 0;
        while (displacement, velocity) != (0.0, 0.0) {
            assert!(
                frames < 120,
                "{} comes to rest within two seconds: {displacement}, {velocity}",
                movement.name()
            );
            spring.advance(&mut displacement, &mut velocity, &mut passed, 1.0 / 60.0);
            assert!(displacement.is_finite() && velocity.is_finite());
            frames += 1;
        }
    }
}
