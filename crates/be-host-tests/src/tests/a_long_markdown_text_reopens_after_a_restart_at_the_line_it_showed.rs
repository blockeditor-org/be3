use super::*;

const SECTIONS: usize = 40;

/// The first row shown whole at the top of the text: its words, and where it is.
fn top_row(host: &mut HostTest) -> (String, f32) {
    let (_, top, _, _) = host.place("#text.surface");
    let tree = host.tree();
    tree.lines()
        .filter_map(|line| {
            let at = line.find("Label value=\"")? + "Label value=\"".len();
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
        .filter(|(name, y)| *y >= top && name.contains(' '))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("the text shows its rows")
}

/// A document of rows of many heights (headings, wrapped paragraphs, lists, code blocks), whose
/// heights a text area estimates before it lays them out: after a restart the rows above the
/// one it reopens at are laid out after it, and must not push it away.
#[test]
fn a_long_markdown_text_reopens_after_a_restart_at_the_line_it_showed() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    host.run(&["resize", "390x844"]);
    host.click("#text.surface");
    let text = (1..=SECTIONS)
        .map(|section| {
            format!(
                "## Section {section}\n\nParagraph {section} with **bold** and *italic* words \
                 that wrap across the narrow phone screen several times over.\n\n- item \
                 {section}.a\n- item {section}.b\n\n```rust\nfn main() {{}}\n```\n"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    host.run(&["clipboard", &text]);
    host.key("ctrl+v");
    host.key("Escape");
    host.run(&["scroll", "#text.surface", "-4000"]);
    host.pause(1000);
    host.run(&["settle"]);
    let left = top_row(&mut host);
    assert!(
        !left.0.contains("Section 1 ") && !left.0.contains(&format!("{SECTIONS}.")),
        "the text is scrolled to its middle: {left:?}"
    );

    host.click("#dock.back");
    host.wait("Pane \"Files\"");
    host.restart();
    host.run(&["resize", "390x844"]);
    host.wait("TreeItem \"## Section 1\"");
    host.click("TreeItem \"## Section 1\"");
    host.wait("#text.surface");
    host.gone("#editor.loading");
    host.run(&["settle"]);
    let reopened = top_row(&mut host);
    assert!(
        reopened.0 == left.0 && (reopened.1 - left.1).abs() <= 1.5,
        "after a restart the text opens with the same row at the top, to the pixel it is \
         snapped to: {reopened:?} against {left:?}"
    );
    host.close();
}
