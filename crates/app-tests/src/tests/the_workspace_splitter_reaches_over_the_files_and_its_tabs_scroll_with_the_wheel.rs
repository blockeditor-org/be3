use super::*;

const SPLITTER: &str = "#dock.splitter.workspace";
const FILES_TAB: &str = "#dock.tab.1";

fn at(x: f32, y: f32) -> String {
    format!("{x:.0},{y:.0}")
}

#[test]
fn the_workspace_splitter_reaches_over_the_files_and_its_tabs_scroll_with_the_wheel() {
    let mut host = HostTest::start();
    host.wait(SPLITTER);
    host.wait("#file-tree.shared.row");
    host.run(&["settle"]);

    let (x, y, _, height) = host.place(SPLITTER);
    let inside_files = at(x + 1.0, y + height / 2.0);
    host.run(&["move", &inside_files]);
    let state = host.run(&["state"]);
    assert!(
        state.contains("cursor ResizeHorizontal"),
        "the splitter takes the pointer where it reaches over the files, not the files beneath it:\n{state}"
    );
    host.snapshot("the_workspace_splitter_reaches_over_the_files_hovered");

    let tab = host.place(FILES_TAB);
    let over_tabs = at(tab.0 + tab.2, tab.1 + tab.3 / 2.0);
    host.run(&["wheel", &over_tabs, "2"]);
    host.run(&["settle"]);
    let scrolled = host.place(FILES_TAB);
    assert!(
        scrolled.0 < tab.0,
        "a vertical wheel over the tab bar scrolls it sideways, {tab:?} to {scrolled:?}"
    );
    host.snapshot("the_workspace_splitter_reaches_over_the_files_tabs_scrolled");

    host.run(&["drag", &inside_files, &at(x + 41.0, y + height / 2.0)]);
    host.run(&["settle"]);
    let moved = host.place(SPLITTER);
    assert!(
        (moved.0 - (x + 40.0)).abs() <= 1.0,
        "a drag that starts over the files' edge moves the splitter, from {x} to {}",
        moved.0
    );
    host.key("Left");
    host.run(&["move", "600,400"]);
    host.run(&["settle"]);
    host.snapshot("the_workspace_splitter_reaches_over_the_files_focused");
    host.close();
}
