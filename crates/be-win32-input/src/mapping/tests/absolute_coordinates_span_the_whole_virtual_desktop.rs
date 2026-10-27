use super::*;

#[test]
fn absolute_coordinates_span_the_whole_virtual_desktop() {
    let desktop = PixelRect::new(-1920, -200, 1920 + 2560, 1440 + 200);
    let first = absolute(desktop, PixelPoint::new(-1920, -200));
    let last = absolute(desktop, PixelPoint::new(2559, 1439));
    assert!(first.x <= 10 && first.y <= 30, "{first:?}");
    assert!(last.x >= 65525 && last.y >= 65505, "{last:?}");
    assert!(last.x <= ABSOLUTE_MAX && last.y <= ABSOLUTE_MAX);
    assert_eq!(
        absolute(desktop, PixelPoint::new(-5000, 9000)),
        absolute(desktop, PixelPoint::new(-1920, 1439))
    );
}
