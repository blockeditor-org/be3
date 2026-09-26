use super::*;

#[test]
fn templates_are_grouped_under_the_editor_that_declares_them() {
    let sections = template_sections(&registry(), &HashSet::from([uuid(CANVAS)]));

    assert_eq!(sections.len(), 1);
    let deck = &sections[0];
    assert_eq!(deck.title, "Presentation");
    assert_eq!(deck.icon.as_deref(), Some("a"));
    let tiles: Vec<_> = deck
        .tiles
        .iter()
        .map(|tile| {
            (
                tile.label.as_str(),
                tile.icon.as_str(),
                tile.action.template,
            )
        })
        .collect();
    assert_eq!(
        tiles,
        [
            ("Title page", "t", "title-slide"),
            ("Blank page", "b", "blank-slide"),
        ]
    );
    assert!(
        deck.tiles
            .iter()
            .all(|tile| tile.action.editor == uuid(DECK))
    );
}
