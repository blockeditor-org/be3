use super::*;

#[test]
fn a_window_ask_goes_to_the_region_of_the_instance_that_made_it() {
    let second = shown(2);
    let ask = WindowRequest::Raise(ViewId(7));
    let mut driver = driven(Plain {
        asks: vec![(second.id(), ask.clone())],
        ..Plain::default()
    });
    let frames = driver.frame(&[shown(1), second]);
    assert!(frames[0].asks.is_empty());
    assert_eq!(frames[1].asks, vec![ask]);
}
