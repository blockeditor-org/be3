use super::*;

#[test]
fn typing_in_the_apps_panel_keeps_its_search_field_still() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.set_watched::<ProgramsWatch>(&a_shelf_of_programs());
    fixture.test.click("desktop.apps");
    fixture.settle();
    let field = fixture.test.rect_of("desktop.apps.search");
    let panel = fixture.test.rect_of("desktop.apps.panel");
    for letter in ["l", "i", "b", "r", "e", "w"] {
        fixture.test.type_text(letter);
        fixture.settle();
        assert_eq!(
            fixture.test.rect_of("desktop.apps.search"),
            field,
            "typing {letter:?} moved the search field"
        );
        assert_eq!(
            fixture.test.rect_of("desktop.apps.panel"),
            panel,
            "typing {letter:?} resized the panel"
        );
    }
    assert!(fixture.test.shown("desktop.apps.item.libreoffice-writer.desktop"));
    assert!(!fixture.test.shown("desktop.apps.item.libreoffice-calc.desktop"));
    fixture.test.snapshot("the_apps_panel_narrowed_to_one_program");
}
