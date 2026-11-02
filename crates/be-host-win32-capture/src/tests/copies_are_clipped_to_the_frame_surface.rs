use super::*;

#[test]
fn copies_are_clipped_to_the_frame_surface() {
    let content = Size {
        width: 1920,
        height: 900,
    };
    let surface = Size {
        width: 1600,
        height: 1200,
    };
    assert_eq!(
        copied(content, surface),
        Size {
            width: 1600,
            height: 900
        }
    );
    assert_eq!(Size::of(0, 10), None);
    assert_eq!(Size::of(10, -1), None);
}
