use super::*;

#[test]
fn every_pixel_comes_back_from_its_absolute_coordinates() {
    for desktop in [
        PixelRect::new(0, 0, 1920, 1080),
        PixelRect::new(-1280, -1024, 1280 + 3840, 1024 + 2160),
        PixelRect::new(-7, 3, 3, 1),
    ] {
        for x in [0, 1, desktop.width / 2, desktop.width - 1] {
            for y in [0, desktop.height / 3, desktop.height - 1] {
                let pixel = PixelPoint::new(desktop.left + x, desktop.top + y);
                let mapped = absolute(desktop, pixel);
                assert_eq!(from_absolute(desktop, mapped), pixel, "{desktop:?}");
            }
        }
    }
}
