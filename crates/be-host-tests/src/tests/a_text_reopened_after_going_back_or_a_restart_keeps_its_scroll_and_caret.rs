use super::*;

const LINES: usize = 80;

/// The first line shown whole at the top of the text, and where it is.
fn top_line(host: &mut HostTest) -> (String, f32) {
    let (_, top, _, _) = host.place("#text.surface");
    let tree = host.tree();
    tree.lines()
        .filter_map(|line| {
            let at = line.find("Label value=\"Line ")? + "Label value=\"".len();
            let name = line[at..].split('"').next()?.to_owned();
            let y: f32 = line
                .split(" at ")
                .nth(1)?
                .split(',')
                .nth(1)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()?;
            Some((name, y))
        })
        .filter(|(_, y)| *y >= top)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("the text shows its lines")
}

fn reopen(host: &mut HostTest) {
    host.click("TreeItem \"Line 1\"");
    host.wait("#text.surface");
    host.gone("#editor.loading");
    host.run(&["settle"]);
}

/// Types where the caret is without moving it, by focusing the text as a screen reader does,
/// and answers whether the last line then reads `last`.
fn type_at_caret(host: &mut HostTest, typed: &str, last: &str) -> bool {
    host.run(&["a11y", "MultilineTextInput", "focus"]);
    host.type_text(typed);
    host.run(&["settle"]);
    host.tree().contains(&format!("Line {LINES}{last}"))
}

fn back(host: &mut HostTest) {
    host.click("#dock.back");
    host.wait("Pane \"Files\"");
}

#[test]
fn a_text_reopened_after_going_back_or_a_restart_keeps_its_scroll_and_caret() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Text");
    host.run(&["resize", "390x800"]);
    host.click("#text.surface");
    let text = (1..=LINES)
        .map(|line| format!("Line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    host.type_text(&text);
    host.key("Escape");
    host.run(&["scroll", "#text.surface", "-500"]);
    host.pause(1000);
    let left = top_line(&mut host);
    assert_ne!(left.0, "Line 1", "the text is scrolled away from its start");

    back(&mut host);
    reopen(&mut host);
    assert_eq!(
        top_line(&mut host),
        left,
        "going back on a phone closes the text's tab, and opening it again from the files shows \
         the same line at the top"
    );
    assert!(
        type_at_caret(&mut host, "A", "A"),
        "the caret is where it was left, at the end"
    );
    // Typing scrolled to the end, which shows less with the format bar down; move up from it.
    host.key("Escape");
    host.run(&["scroll", "#text.surface", "-300"]);
    host.pause(1000);
    let typed = top_line(&mut host);

    back(&mut host);
    host.restart("Pane \"Files\"");
    host.run(&["resize", "390x800"]);
    host.wait("TreeItem \"Line 1\"");
    reopen(&mut host);
    assert_eq!(
        top_line(&mut host),
        typed,
        "after a restart the text opens at the same line too"
    );
    assert!(
        type_at_caret(&mut host, "B", "AB"),
        "with the caret where it was"
    );
    host.close();
}
