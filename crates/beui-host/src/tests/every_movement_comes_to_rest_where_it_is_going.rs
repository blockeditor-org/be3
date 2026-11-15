use crate::styled::Movement;

#[test]
fn every_movement_comes_to_rest_where_it_is_going() {
    for movement in Movement::ALL {
        let easing = movement.easing();
        assert_eq!(easing.sample(0.0), 0.0, "{} starts where it was", movement.name());
        assert_eq!(easing.sample(1.0), 1.0, "{} ends where it goes", movement.name());

        let spring = movement.spring();
        let (mut displacement, mut velocity) = (1.0_f32, 0.0_f32);
        for _ in 0..120 {
            spring.advance(&mut displacement, &mut velocity, 1.0 / 60.0);
            assert!(displacement.is_finite() && velocity.is_finite());
        }
        assert!(
            displacement.abs() < 1e-3 && velocity.abs() < 1e-2,
            "{} has come to rest within two seconds: {displacement}, {velocity}",
            movement.name()
        );
    }
}
