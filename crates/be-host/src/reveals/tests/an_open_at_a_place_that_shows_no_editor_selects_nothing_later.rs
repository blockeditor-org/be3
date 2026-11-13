use super::*;

#[test]
fn an_open_at_a_place_that_shows_no_editor_selects_nothing_later() {
    let block = Uuid::new_v4();
    let place = SearchLocation {
        key: String::new(),
        text: "late".to_owned(),
    };
    let mut reveals = Reveals::default();
    reveals.opened(block, Some(place.clone()), Duration::ZERO);
    let mut told = Vec::new();
    reveals.deliver(Duration::from_secs(1), |_, _| Delivery::NoEditor);
    assert!(!reveals.is_empty(), "it waits a while for the editor");

    reveals.deliver(PATIENCE, |_, _| Delivery::NoEditor);
    assert!(
        told.is_empty() && reveals.is_empty(),
        "an open that never showed an editor lets go of its place: {told:?}"
    );

    reveals.opened(block, None, PATIENCE);
    reveals.deliver(PATIENCE, |block, at| {
        told.push((block, at.clone()));
        Delivery::Told
    });
    assert!(told.is_empty(), "a plain open later selects nothing");

    reveals.opened(block, Some(place.clone()), PATIENCE);
    reveals.deliver(PATIENCE, |block, at| {
        told.push((block, at.clone()));
        Delivery::Told
    });
    assert_eq!(
        told,
        [(block, place)],
        "an open whose editor is there gets its place"
    );
    assert!(reveals.is_empty());
}
