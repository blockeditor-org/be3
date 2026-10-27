use super::*;

#[test]
fn a_move_goes_to_the_point_in_virtual_desktop_coordinates() {
    let synths = pointer(DESKTOP, PixelPoint::new(1000, 500), PointerAction::Move);
    let [only] = synths[..] else {
        panic!("{synths:?}");
    };
    let (x, y, flags) = moved_to(only);
    assert_eq!(
        flags,
        MOUSE_MOVE | MOUSE_ABSOLUTE | MOUSE_VIRTUAL_DESK,
        "{flags:#x}"
    );
    assert_eq!((x, y), (32784, 32800));
}
