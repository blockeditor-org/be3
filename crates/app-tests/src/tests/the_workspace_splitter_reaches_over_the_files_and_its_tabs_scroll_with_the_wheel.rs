use super::*;

const SPLITTER: &str = "#dock.splitter.workspace";
const FILES_TAB: &str = "#dock.tab.1";

fn at(x: f32, y: f32) -> String {
    format!("{x:.0},{y:.0}")
}

#[test]
fn the_workspace_splitter_reaches_over_the_files_and_its_tabs_scroll_with_the_wheel() {
    let mut app = AppTest::start();
    app.wait(SPLITTER);
    app.wait("#file-tree.shared.row");
    app.run(&["settle"]);

    let (x, y, _, height) = app.place(SPLITTER);
    let inside_files = at(x + 1.0, y + height / 2.0);
    app.run(&["move", &inside_files]);
    let state = app.run(&["state"]);
    assert!(
        state.contains("cursor ResizeHorizontal"),
        "the splitter takes the pointer where it reaches over the files, not the files beneath it:\n{state}"
    );
    app.snapshot("the_workspace_splitter_reaches_over_the_files_hovered");

    let tab = app.place(FILES_TAB);
    let over_tabs = at(tab.0 + tab.2, tab.1 + tab.3 / 2.0);
    app.run(&["wheel", &over_tabs, "2"]);
    app.run(&["settle"]);
    let scrolled = app.place(FILES_TAB);
    assert!(
        scrolled.0 < tab.0,
        "a vertical wheel over the tab bar scrolls it sideways, {tab:?} to {scrolled:?}"
    );
    app.snapshot("the_workspace_splitter_reaches_over_the_files_tabs_scrolled");

    app.run(&["drag", &inside_files, &at(x + 41.0, y + height / 2.0)]);
    app.run(&["settle"]);
    let moved = app.place(SPLITTER);
    assert!(
        (moved.0 - (x + 40.0)).abs() <= 1.0,
        "a drag that starts over the files' edge moves the splitter, from {x} to {}",
        moved.0
    );
    app.key("Left");
    app.run(&["move", "600,400"]);
    app.run(&["settle"]);
    app.snapshot("the_workspace_splitter_reaches_over_the_files_focused");
    app.close();
}
