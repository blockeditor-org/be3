use super::*;

/// A canvas's dock joins its tab. The live canvas is kept while a revision is previewed, and its
/// dock lets the tab go meanwhile, so the preview shows under its revision bar and the canvas comes
/// back as it was.
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
    // The block just added is selected: the live canvas keeps its selection through a preview.
    host.wait("#infinite-canvas.selection-bar");
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
    // The live canvas comes back as it was, kept rather than built again: its dock joined the
    // tab again (its tool bar shows) and the block it had selected still is.
    host.wait("#infinite-canvas.dock");
    host.wait("#infinite-canvas.selection-bar");
    host.close();
}
