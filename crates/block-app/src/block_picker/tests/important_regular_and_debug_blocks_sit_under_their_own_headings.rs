use super::*;

#[test]
fn important_regular_and_debug_blocks_sit_under_their_own_headings() {
    let sections = add_sections(&registry(), &HashSet::new());

    let shown: Vec<(String, Vec<String>)> = sections
        .iter()
        .map(|section| {
            let labels = section
                .tiles
                .iter()
                .map(|tile| tile.label.clone())
                .collect();
            (section.title.clone(), labels)
        })
        .collect();
    assert_eq!(
        shown,
        [
            ("Common".to_owned(), vec!["Canvas".to_owned()]),
            ("More blocks".to_owned(), vec!["Presentation".to_owned()]),
            ("Debug".to_owned(), vec!["Counter".to_owned()]),
        ]
    );

    let only_canvases = add_sections(&registry(), &HashSet::from([uuid(CANVAS)]));
    assert_eq!(only_canvases.len(), 1);
    assert_eq!(only_canvases[0].title, "Common");
}
