use super::*;

#[test]
fn a_program_tile_whose_name_is_cut_short_shows_it_whole_on_hover() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.set_watched::<ProgramsWatch>(&a_shelf_of_programs());
    fixture.test.click("desktop.apps");
    fixture.settle();
    let calc = "desktop.apps.item.libreoffice-calc.desktop";
    assert_eq!(
        accessible_label_around(&fixture, calc).as_deref(),
        Some("LibreOffice Calc"),
        "a tile is named in full to assistive technology"
    );

    fixture.test.hover("desktop.apps.item.files.desktop");
    fixture.settle();
    fixture.test.advance(std::time::Duration::from_secs(2));
    fixture.test.settle();
    assert!(
        !fixture.test.shown("tooltip"),
        "a name shown whole needs no tooltip"
    );

    fixture.test.hover(calc);
    fixture.settle();
    fixture.test.advance(std::time::Duration::from_secs(2));
    fixture.test.settle();
    assert!(
        fixture.test.shown("tooltip"),
        "a name cut short shows whole in a tooltip"
    );
    assert_eq!(fixture.test.label("tooltip"), "LibreOffice Calc");
    fixture.test.snapshot("a_cut_short_program_name_in_a_tooltip");
}
