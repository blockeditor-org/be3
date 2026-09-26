use super::*;

#[test]
fn images_in_comments_and_tables_are_gathered_in_order() {
    let comment = |body: &str, inline: Vec<InlineComment>| Entry::Comment {
        author: Person {
            login: "ada".to_owned(),
            avatar: String::new(),
        },
        verb: "commented".to_owned(),
        tone: Tone::Neutral,
        when: 0,
        url: String::new(),
        body: blocks(body),
        inline,
    };
    let entries = vec![
        comment("![](https://example.com/a.png)", Vec::new()),
        Entry::Commit {
            sha: "0123456".to_owned(),
            message: "fix".to_owned(),
        },
        comment(
            "| Before | After |\n| --- | --- |\n| ![](https://example.com/b.png) | ![](https://example.com/c.png) |",
            vec![InlineComment {
                author: "grace".to_owned(),
                location: "src/lib.rs:1".to_owned(),
                body: blocks("<img src=\"https://example.com/d.png\">"),
            }],
        ),
    ];
    assert_eq!(
        timeline_images(&entries),
        ["a", "b", "c", "d"].map(|name| format!("https://example.com/{name}.png"))
    );
}
