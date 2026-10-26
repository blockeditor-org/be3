use super::*;

#[test]
fn a_band_let_go_heading_back_carries_its_speed_past_the_edge() {
    let frame = 1.0 / 120.0;
    let (mut stretch, mut velocity) = (-20.0, 3000.0);
    let mut back = 0.0;
    for _ in 0..30 {
        back = bounce(&mut stretch, &mut velocity, frame);
        if stretch == 0.0 {
            break;
        }
    }
    assert_eq!(stretch, 0.0);
    assert!(
        back > 1000.0,
        "the speed it was let go with carries on into the content: {back}"
    );
}
