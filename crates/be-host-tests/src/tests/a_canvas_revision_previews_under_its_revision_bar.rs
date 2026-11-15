use super::*;

/// A canvas's dock joins its tab, so the live canvas kept alive beside a preview used to keep its
/// panes over the whole tab, hiding the preview and the revision bar.
#[test]
fn a_canvas_revision_previews_under_its_revision_bar() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.dock");
    // History keeps one state per minute: the empty canvas's minute passes before the edit.
    host.pause(60_000);
    host.synced();
    host.run(&["scroll", "Tab \"Settings\"", "0", "200"]);
    host.click("Tab \"History\"");
    host.wait("UTC · Current");
    let original = host
        .tree()
        .lines()
        .find(|line| line.contains("Current") && line.contains("#workspace.revision."))
        .and_then(|line| line.split('#').nth(1))
        .and_then(|tail| tail.split_whitespace().next())
        .map(|id| format!("#{id}"))
        .expect("the empty canvas has a saved revision");
    host.pause(60_000);
    add_to_canvas(&mut host, "Text");
    host.wait(&format!("UTC\" {original}"));
    host.synced();
    host.click(&original);
    host.wait("#workspace.revision.current");
    host.wait("#workspace.revision.preview");
    let (_, bar_y, _, bar_h) = host.place("#workspace.revision.current");
    let (_, preview_y, _, _) = host.place("#workspace.revision.preview");
    assert!(
        preview_y >= bar_y + bar_h,
        "the preview shows under the revision bar, at {preview_y}, the bar ending at {}",
        bar_y + bar_h
    );
    host.click("#workspace.revision.current");
    host.gone("#workspace.revision.preview");
    host.wait("GenericContainer #infinite-canvas.entity.");
    host.close();
}
