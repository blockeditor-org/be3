use super::*;

#[test]
fn revision_history_previews_and_restores_a_text_file() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    app.click("#text.surface");
    app.type_text("Original revision");
    app.run(&["app", "scroll", "Tab \"Settings\"", "0", "200"]);
    app.click("Tab \"History\"");
    app.wait("UTC\" #workspace.revision.");
    app.wait("UTC · Current");
    let original = app
        .tree()
        .lines()
        .find(|line| line.contains("Current") && line.contains("#workspace.revision."))
        .and_then(|line| line.split('#').nth(1))
        .and_then(|tail| tail.split_whitespace().next())
        .map(|id| format!("#{id}"))
        .expect("the original content has a saved revision");
    app.click("#text.surface");
    app.key("ctrl+a");
    app.type_text("Latest revision");
    app.wait("Latest revision");
    app.wait(&format!("UTC\" {original}"));
    app.click(&original);
    app.wait("#workspace.revision.current");
    app.wait("Original revision");
    app.crop("#workspace.revision.preview");
    app.snapshot("revision_history_previews_a_saved_text_file");
    app.click("#text.surface");
    app.key("ctrl+a");
    app.type_text("This must not change the revision");
    assert!(!app.tree().contains("This must not change the revision"));
    app.click("#workspace.revision.restore");
    app.click("#workspace.revision.confirm");
    app.gone("#workspace.revision.current");
    app.wait("Original revision");
    app.close();
}
