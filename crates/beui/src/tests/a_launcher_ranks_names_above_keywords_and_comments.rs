use crate::styled::{LauncherItem, launcher_rank};

fn item(key: &str, title: &str, detail: &str, terms: &[&str]) -> LauncherItem {
    LauncherItem {
        key: key.to_owned(),
        title: title.to_owned(),
        detail: detail.to_owned(),
        terms: terms.iter().map(|term| (*term).to_owned()).collect(),
        image: None,
    }
}

fn ranked(items: &[LauncherItem], query: &str) -> Vec<String> {
    launcher_rank(items, query)
        .into_iter()
        .map(|index| items[index].key.clone())
        .collect()
}

#[test]
fn a_launcher_ranks_names_above_keywords_and_comments() {
    let items = [
        item(
            "files",
            "Files",
            "Access and organize files",
            &["File Manager", "folder"],
        ),
        item(
            "foot",
            "Foot",
            "A fast Wayland terminal",
            &["Terminal", "shell"],
        ),
        item(
            "text",
            "Text Editor",
            "Edit text files",
            &["gedit", "notepad"],
        ),
        item(
            "term",
            "Terminal",
            "Use the command line",
            &["shell", "prompt"],
        ),
        item(
            "firefox",
            "Firefox Web Browser",
            "Browse the web",
            &["internet"],
        ),
    ];
    assert_eq!(
        ranked(&items, ""),
        ["files", "foot", "text", "term", "firefox"],
        "an empty query lists everything in the order given"
    );
    assert_eq!(
        ranked(&items, "term"),
        ["term", "foot"],
        "a name beats a generic name, and a comment alone does not match a scattered word"
    );
    assert_eq!(ranked(&items, "TERMINAL"), ["term", "foot"]);
    assert_eq!(
        ranked(&items, "fi"),
        ["files", "firefox", "text"],
        "a name's prefix beats a word inside a comment"
    );
    assert_eq!(
        ranked(&items, "ffx"),
        ["firefox"],
        "the letters of a name can be scattered"
    );
    assert_eq!(
        ranked(&items, "web browser"),
        ["firefox"],
        "every word has to match"
    );
    assert_eq!(ranked(&items, "notepad"), ["text"], "keywords match");
    assert!(ranked(&items, "zzz").is_empty());
}
