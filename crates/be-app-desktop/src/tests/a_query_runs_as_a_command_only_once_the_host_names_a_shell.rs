use super::*;

#[test]
fn a_query_runs_as_a_command_only_once_the_host_names_a_shell() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.set_watched::<ProgramsWatch>(&ProgramCatalog {
        shell: Vec::new(),
        ..a_shelf_of_programs()
    });
    fixture.test.click("desktop.apps");
    fixture.settle();
    fixture.test.type_text("weston-simple-shm");
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.apps.item.run"),
        "with no shell named, a typed line has nothing to run through"
    );

    fixture
        .test
        .set_watched::<ProgramsWatch>(&a_shelf_of_programs());
    fixture.settle();
    assert!(fixture.test.shown("desktop.apps.item.run"));
}
