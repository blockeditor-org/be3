use super::*;

#[test]
fn a_hidden_session_chosen_from_the_apps_panel_comes_back_in_front_with_the_focus() {
    let (mut fixture, sessions) =
        Fixture::new().with_sessions(&[("Laptop", WORKSPACE_EDITOR), ("Desk", WORKSPACE_EDITOR)]);
    fixture.settle();
    for session in &sessions {
        fixture.test.click("desktop.apps");
        fixture.settle();
        fixture
            .test
            .click(&format!("desktop.apps.sessions.{session}"));
        fixture.settle();
    }
    assert_eq!(fixture.placed_blocks().len(), 2, "both sessions open");

    let chip = "desktop.windows.block.2";
    fixture.test.click(chip);
    fixture.settle();
    fixture.test.click(chip);
    fixture.settle();
    let placed = fixture.placed_blocks();
    let [(block, _, _)] = placed[..] else {
        panic!("a click on the focused session's chip hides it: {placed:?}");
    };
    assert_eq!(block, sessions[1], "the other session stays");

    fixture.test.click("desktop.apps");
    fixture.settle();
    assert!(fixture.says("Hidden"), "a hidden session says so");
    assert!(fixture.says("Open"), "and the shown one says it is open");
    fixture
        .test
        .snapshot("the_apps_panel_with_a_hidden_session");
    fixture
        .test
        .click(&format!("desktop.apps.sessions.{}", sessions[0]));
    fixture.settle();
    let placed = fixture.placed_blocks();
    assert_eq!(
        placed.len(),
        2,
        "choosing the hidden session shows its window, and no second one: {placed:?}"
    );
    assert!(
        !fixture.test.shown("desktop.apps.sessions.new"),
        "choosing it closes the panel"
    );

    fixture.test.click(chip);
    fixture.settle();
    let placed = fixture.placed_blocks();
    let [(block, _, _)] = placed[..] else {
        panic!("it came back with the focus, so its chip hides it again: {placed:?}");
    };
    assert_eq!(block, sessions[1]);
}
