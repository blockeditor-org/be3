use super::*;

#[test]
fn the_band_gives_way_at_the_speed_it_is_pulled_then_stiffens() {
    let dimension = 600.0;
    let slope = |at: f32| (rubber_band(at + 0.5, dimension) - rubber_band(at, dimension)) / 0.5;
    assert!(
        slope(0.0) > 0.99,
        "content keeps up with the fingers as it crosses the edge: {}",
        slope(0.0)
    );
    let mut was = slope(0.0);
    for at in (1..60).map(|step| step as f32 * 10.0) {
        let now = slope(at);
        assert!(
            now < was,
            "the band stiffens the further it is pulled: {now} at {at}"
        );
        was = now;
        let back = unband(rubber_band(-at, dimension), dimension);
        assert!(
            (back + at).abs() < 0.01,
            "unbanding undoes it: {back} for {at}"
        );
    }
}
