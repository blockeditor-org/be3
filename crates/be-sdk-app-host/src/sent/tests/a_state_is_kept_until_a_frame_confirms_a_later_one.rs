use super::*;

#[test]
fn a_state_is_kept_until_a_frame_confirms_a_later_one() {
    let mut sent = Sent::default();
    sent.push(1, "narrow");
    sent.push(2, "narrow");
    sent.push(3, "wide");
    sent.push(5, "tall");
    assert_eq!(sent.len(), 3, "a serial that changed nothing adds no state");

    assert_eq!(sent.at(0), None, "nothing was sent before the first serial");
    assert_eq!(sent.at(2), Some(&"narrow"));
    assert_eq!(sent.at(4), Some(&"wide"));
    assert_eq!(sent.at(9), Some(&"tall"));

    sent.confirm(4);
    assert_eq!(
        sent.len(),
        2,
        "once a frame laid out against serial 4 arrives, nothing before the state it used is needed"
    );
    assert_eq!(sent.at(4), Some(&"wide"));
    assert_eq!(sent.at(2), None);

    sent.confirm(5);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent.at(5), Some(&"tall"));
}
