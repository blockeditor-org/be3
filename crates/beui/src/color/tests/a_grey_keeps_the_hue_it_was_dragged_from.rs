use super::*;

#[test]
fn a_grey_keeps_the_hue_it_was_dragged_from() {
    let previous = Hsva::new(200.0, 0.8, 0.5, 1.0);
    let black = Hsva::from_color_keeping(Color32::BLACK, previous);
    assert_eq!((black.hue, black.saturation, black.value), (200.0, 0.8, 0.0));
    let grey = Hsva::from_color_keeping(Color32::from_gray(128), previous);
    assert_eq!((grey.hue, grey.saturation), (200.0, 0.0));
    assert_eq!(Hsva::from_color_keeping(previous.to_color(), previous), previous);
}
