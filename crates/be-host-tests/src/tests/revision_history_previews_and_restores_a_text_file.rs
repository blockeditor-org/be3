use super::*;

#[test]
fn revision_history_previews_and_restores_a_text_file() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Text");
    host.click("#text.surface");
    host.type_text("Original revision");
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
        .expect("the original content has a saved revision");
    // History keeps a state per minute, so the original stays only once the next edit lands a
    // minute later.
    host.pause(60_000);
    host.click("#text.surface");
    host.key("ctrl+a");
    host.type_text("Latest revision");
    host.wait("Latest revision");
    host.wait(&format!("UTC\" {original}"));
    host.click(&original);
    host.wait("#workspace.revision.current");
    host.wait("Original revision");
    host.crop("#workspace.revision.preview");
    host.snapshot("revision_history_previews_a_saved_text_file");
    host.click("#text.surface");
    host.key("ctrl+a");
    host.type_text("This must not change the revision");
    assert!(!host.tree().contains("This must not change the revision"));
    host.click("#workspace.revision.restore");
    host.click("#workspace.revision.confirm");
    host.gone("#workspace.revision.current");
    host.wait("Original revision");
    host.close();
}
