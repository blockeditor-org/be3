use super::*;

#[test]
fn a_demo_tab_unlocked_from_its_grip_moves_beside_the_files_with_its_page() {
    let mut app = AppTest::start();
    open_beui_demo(&mut app);
    let ids = app.run(&["ids"]);
    let grip = ids
        .split_whitespace()
        .map(|id| id.trim_start_matches('#'))
        .find(|id| id.starts_with("dock.grip.") && id.ends_with("/pages"))
        .map(|id| format!("#{id}"))
        .expect("the demo's pages pane has a grip in the workspace dock");
    app.run(&["click", &grip, "right"]);
    app.click("MenuItemCheckBox \"Locked\"");
    app.gone("MenuItemCheckBox \"Locked\"");
    let files = app.place("Pane \"Files\"");
    let below = format!(
        "{:.0},{:.0}",
        files.0 + files.2 * 0.5,
        files.1 + files.3 * 0.9
    );
    app.run(&["drag", "Tab \"Docking\"", &below]);
    app.run(&["settle"]);
    let tab = app.place("Tab \"Docking\"");
    assert!(
        tab.0 < files.0 + files.2 && tab.1 > files.1 + files.3 * 0.25,
        "the unlocked tab moved below the files, to {tab:?}"
    );
    app.wait("Tab \"Components\"");
    app.snapshot("a_demo_tab_unlocked_from_its_grip_moves_beside_the_files_with_its_page");
    app.close();
}
