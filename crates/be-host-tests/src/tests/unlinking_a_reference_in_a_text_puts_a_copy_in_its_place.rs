use super::*;

/// Where a part of the `nth` row titled `title` is (0 for the first), by the end of its test id,
/// such as `.more` or `.chevron`, as a target: a reference row shares its block's test ids, so
/// the part is named by the place the tree gives it.
fn row_part(host: &mut HostTest, title: &str, nth: usize, part: &str) -> String {
    let tree = host.tree();
    let row = format!("TreeItem \"{title}\"");
    let mut lines = tree.lines();
    for _ in 0..=nth {
        lines
            .by_ref()
            .find(|line| line.contains(&row))
            .unwrap_or_else(|| panic!("no row {nth} titled {title:?} in\n{tree}"));
    }
    let line = lines
        .take_while(|line| !line.contains("TreeItem "))
        .find(|line| {
            line.split_whitespace()
                .any(|word| word.starts_with("#file-tree.") && word.ends_with(part))
        })
        .unwrap_or_else(|| panic!("row {nth} titled {title:?} has no {part} in\n{tree}"));
    let place = |after: &str, separator: char| -> (f32, f32) {
        let (_, rest) = line.split_once(after).expect("the tree gives each node a place");
        let pair = rest.split_whitespace().next().unwrap_or_default();
        let (a, b) = pair.split_once(separator).expect("a place is two numbers");
        (a.parse().unwrap(), b.parse().unwrap())
    };
    let ((x, y), (width, height)) = (place(" at ", ','), place(" size ", 'x'));
    format!("{},{}", x + width / 2.0, y + height / 2.0)
}

/// The middle of the `nth` row titled `title`'s expand button, which sits at its left end.
fn chevron_of(host: &mut HostTest, title: &str, nth: usize) -> String {
    let tree = host.tree();
    let row = format!("TreeItem \"{title}\"");
    let line = tree
        .lines()
        .filter(|line| line.contains(&row))
        .nth(nth)
        .unwrap_or_else(|| panic!("no row {nth} titled {title:?} in\n{tree}"));
    let (_, rest) = line.split_once(" at ").expect("the tree gives each node a place");
    let (x, rest) = rest.split_once(',').expect("a place is two numbers");
    let y = rest.split_whitespace().next().unwrap_or_default();
    let (_, size) = line.split_once(" size ").expect("the tree gives each node a size");
    let height: f32 = size.split('x').nth(1).and_then(|h| h.split_whitespace().next()).unwrap_or("22").parse().unwrap();
    let (x, y): (f32, f32) = (x.parse().unwrap(), y.parse().unwrap());
    format!("{},{}", x + height / 2.0, y + height / 2.0)
}

/// Puts the URL of the first row titled `title` at the end of the open text, and waits until the
/// text `container` (the first row titled so) lists it.
fn link_in(host: &mut HostTest, container: &str, url: &str) {
    host.click("#text.surface");
    host.key("ctrl+End");
    host.type_text(&format!(" {url}"));
    host.synced();
    let chevron = chevron_of(host, container, 0);
    host.run(&["wait", "Button \"Expand\"", "over", &chevron]);
}

/// Unlinks the reference listed second under the title `title`, and answers the URL of what is
/// listed there then.
fn unlink_second(host: &mut HostTest, title: &str) -> String {
    let more = row_part(host, title, 1, ".more");
    host.click(&more);
    host.click("MenuItem \"Unlink\"");
    host.synced();
    host.run(&["settle"]);
    url_of(host, title, 1)
}

/// The block URL of the `nth` row titled `title`, as its menu's Copy puts it on the clipboard.
fn url_of(host: &mut HostTest, title: &str, nth: usize) -> String {
    let more = row_part(host, title, nth, ".more");
    host.click(&more);
    host.click("MenuItem \"Copy\"");
    let copied = host.run(&["clipboard"]);
    copied
        .split_whitespace()
        .last()
        .unwrap_or_else(|| panic!("Copy put no URL on the clipboard: {copied:?}"))
        .to_owned()
}

fn add_text(host: &mut HostTest, name: &str) {
    start_adding_to_my_files(host, "Text");
    host.type_text(name);
    host.key("Enter");
    host.wait(&format!("TreeItem \"{name}\""));
}

/// Unlinking a block a text names by its URL, from the reference listed under the text, puts a
/// copy in its place, whether the text is open or closed: the text names the copy, which is
/// listed under it, and the original stays.
#[test]
fn unlinking_a_reference_in_a_text_puts_a_copy_in_its_place() {
    let mut host = HostTest::start(Options::default());
    add_text(&mut host, "Source");
    add_text(&mut host, "Notes");
    let source = url_of(&mut host, "Source", 0);

    // Open: copying from Source's row showed Source, so Notes is opened again to take the URL.
    host.click("TreeItem \"Notes\"");
    host.run(&["settle"]);
    link_in(&mut host, "Notes", &source);
    let chevron = chevron_of(&mut host, "Notes", 0);
    host.click(&chevron);
    host.run(&["settle"]);
    assert_eq!(url_of(&mut host, "Source", 1), source, "Notes lists Source");
    let open = unlink_second(&mut host, "Source");
    assert_ne!(open, source, "the open text names a copy");
    assert_eq!(url_of(&mut host, "Source", 0), source, "the original stays");

    // Closed: Notes names Source again, then its tab is closed before the unlink.
    host.click("TreeItem \"Notes\"");
    host.run(&["settle"]);
    host.click("#text.surface");
    host.key("ctrl+End");
    host.type_text(&format!(" {source}"));
    host.synced();
    // Every tab is closed, Notes's among them.
    while host.shown("Button \"Close tab\"") {
        let tree = host.tree();
        let close = tree
            .lines()
            .find(|line| line.contains("Button \"Close tab\""))
            .and_then(|line| line.split_whitespace().find(|word| word.starts_with("#dock.tab.")))
            .expect("a tab's close button has a test id")
            .to_owned();
        host.click(&close);
        host.run(&["settle"]);
    }
    host.gone("#text.surface");
    host.run(&["settle"]);
    let listed: Vec<String> = (1..=2).map(|nth| url_of(&mut host, "Source", nth)).collect();
    assert!(listed.contains(&source) && listed.contains(&open), "{listed:?}");
    let reference = 1 + listed.iter().position(|url| *url == source).unwrap_or_default();
    let more = row_part(&mut host, "Source", reference, ".more");
    host.click(&more);
    host.click("MenuItem \"Unlink\"");
    host.synced();
    host.run(&["settle"]);
    let closed: Vec<String> = (1..=2).map(|nth| url_of(&mut host, "Source", nth)).collect();
    assert!(
        !closed.contains(&source) && closed.contains(&open) && !closed.contains(&String::new()),
        "the closed text names a second copy: {closed:?}"
    );
    assert_ne!(closed[0], closed[1], "each unlink made its own copy");
    assert_eq!(url_of(&mut host, "Source", 0), source, "the original stays");
    host.close();
}
