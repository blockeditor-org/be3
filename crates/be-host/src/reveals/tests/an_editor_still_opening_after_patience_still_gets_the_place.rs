use super::*;

#[test]
fn an_editor_still_opening_after_patience_still_gets_the_place() {
    let block = Uuid::new_v4();
    let place = SearchLocation {
        key: String::new(),
        text: "slow".to_owned(),
    };
    let mut reveals = Reveals::default();
    reveals.opened(block, Some(place.clone()), Duration::ZERO);
    reveals.deliver(Duration::from_secs(1), |_, _| Delivery::NoEditor);
    for seconds in [2, 10, 60] {
        reveals.deliver(Duration::from_secs(seconds), |_, _| Delivery::Opening);
        assert!(
            !reveals.is_empty(),
            "an editor still opening at {seconds}s keeps the place waiting"
        );
    }

    let mut told = Vec::new();
    reveals.deliver(Duration::from_secs(61), |block, at| {
        told.push((block, at.clone()));
        Delivery::Told
    });
    assert_eq!(told, [(block, place)], "the editor gets its place once open");
    assert!(reveals.is_empty());
}
