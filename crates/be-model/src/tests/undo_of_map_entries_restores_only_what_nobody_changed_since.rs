use super::*;

#[test]
fn undo_of_map_entries_restores_only_what_nobody_changed_since() {
    let mut document = Document::new(&Sheet {
        cells: [(1, "one".to_owned()), (2, "two".to_owned())]
            .into_iter()
            .collect(),
    });
    let cleared: Edit = Sheet::CELLS.put(ObjectId::ROOT, &1, None).into();
    let cleared_step = document.step(&cleared).expect("clearing changes something");
    document.apply(&cleared);
    let typed: Edit = Sheet::CELLS
        .put(ObjectId::ROOT, &2, Some(&"tw".to_owned()))
        .into();
    let mut typed_step = document.step(&typed).expect("typing changes something");
    document.apply(&typed);
    let more: Edit = Sheet::CELLS
        .put(ObjectId::ROOT, &2, Some(&"twelve".to_owned()))
        .into();
    let more_step = document.step(&more).expect("typing changes something");
    document.apply(&more);
    assert!(typed_step.absorb(more_step).is_ok());
    document.apply(&Sheet::CELLS.put(ObjectId::ROOT, &1, Some(&"uno".to_owned())).into());

    document.apply(&typed_step.undo());
    document.apply(&cleared_step.undo());

    assert_eq!(cell(&document, 1).as_deref(), Some("uno"));
    assert_eq!(cell(&document, 2).as_deref(), Some("two"));
}
