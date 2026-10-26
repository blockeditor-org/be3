use super::*;

#[test]
fn parts_stay_at_their_min_when_the_total_is_short() {
    let parts = [Part::new(1.0, 30.0, 50.0), Part::new(1.0, 40.0, 50.0)];

    assert_eq!(share(50.0, &parts), vec![30.0, 40.0]);
    assert_eq!(share(80.0, &parts), vec![40.0, 40.0]);
    assert_eq!(share(500.0, &parts), vec![50.0, 50.0]);
}
