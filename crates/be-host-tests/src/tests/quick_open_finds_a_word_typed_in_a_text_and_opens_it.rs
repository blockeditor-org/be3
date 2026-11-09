use super::*;

const ROW: &str = "#host.palette.item.";

#[test]
fn quick_open_finds_a_word_typed_in_a_text_and_opens_it() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Text");
    host.click("#text.surface");
    host.type_text("Orchard notes");
    host.key("Enter");
    host.type_text("Quinces ripen late in the autumn");
    // The session seals once it has been quiet a while, and the seal is what gets indexed.
    host.pause(60_000);
    host.run(&["settle"]);
    host.click("#dock.tab.2.close");
    host.run(&["settle"]);
    assert_eq!(host.count("Tab \"Orchard notes\""), 0, "the text is closed");

    host.key("ctrl+p");
    host.wait("#host.palette.search");
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
    assert_eq!(rows.len(), 1, "quick open lists the text by its word: {rows:?}");
    host.snapshot("quick_open_lists_a_text_by_a_word_inside_it");
    host.key("Enter");
    host.gone("#host.palette.search");
    host.run(&["settle"]);
    assert_eq!(
        host.count("Tab \"Orchard notes\""),
        1,
        "the text opens again"
    );
    host.close();
}
