use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

const ROW: &str = "#root.palette.item.";

#[test]
fn be_root_in_place_of_the_hosts_ui_quick_opens_a_text_by_a_word_inside_it() {
    // be-host's own UI writes a text, closes its tab, and waits for the search index to take it
    // in; started again with the root in place of its UI, quick open lists the text by a word
    // inside it, the text around the word under its name, and opens it, as
    // `quick_open_finds_a_word_typed_in_a_text_and_opens_it` does under be-host's.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Text");
    host.click("#text.surface");
    host.type_text("Orchard notes");
    host.key("Enter");
    host.type_text("Quinces ripen late in the autumn");
    // The session seals once it has been quiet a while, and the seal is what gets indexed.
    host.pause(60_000);
    host.run(&["settle"]);
    host.click("#dock.tab.2.close");
    // My files names its search index once its own edits are sealed, which takes another
    // quiet while; the text is indexed then.
    host.pause(60_000);
    host.run(&["settle"]);
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    assert_eq!(host.count("Tab \"Orchard notes\""), 0, "the text is closed");

    // The workspace's own focus, whose Ctrl+P opens quick open: its All tabs button, once its
    // menu closes.
    host.click("#dock.files.all-tabs");
    host.key("Escape");
    host.run(&["settle"]);
    host.key("ctrl+p");
    host.wait("#root.palette.search");
    host.type_text("quinc");
    host.run(&["settle"]);
    host.wait("ripen late");
    let rows: Vec<String> = host
        .tree()
        .lines()
        .filter_map(|line| {
            let at = line.find(ROW)?;
            Some(line[at + ROW.len()..].split_whitespace().next()?.to_owned())
        })
        .collect();
    assert_eq!(
        rows.len(),
        1,
        "quick open lists the text by its word: {rows:?}"
    );
    host.snapshot("be_root_quick_open_lists_a_text_by_a_word_inside_it");
    host.key("Enter");
    host.gone("#root.palette.search");
    host.run(&["settle"]);
    assert_eq!(
        host.count("Tab \"Orchard notes\""),
        1,
        "the text opens again"
    );
    // It opens at the hit, which its editor is told once open (`editor-event.reveal`); under
    // be-host's UI the word quick open found shows selected, under the root not yet
    // (plans/host_as_platform_adapter.md, 4g7g2).
    host.snapshot("be_root_quick_open_opens_a_text_at_the_word_it_found");
    host.close();
}
