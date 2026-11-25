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
        let mut motion = crate::reactive::SpringMotion::new(1000.0, 0.0);
        let mut frames = 0;
        while motion.moving() {
            assert!(
                frames < 120,
                "{} comes to rest within two seconds: {motion:?}",
                movement.name()
            );
            motion.advance(spring, 1.0 / 60.0);
            assert!(motion.displacement.is_finite() && motion.velocity.is_finite());
            frames += 1;
        }
    }
}
