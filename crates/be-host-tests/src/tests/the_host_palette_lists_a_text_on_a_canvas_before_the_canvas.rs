use super::*;

const ROW: &str = "#host.palette.item.";
const TEXT: &str = "be3.text/";
const CANVAS: &str = "be3.infinite-canvas/";
const WORKSPACE: &str = "be3.workspace-ui/";

/// The keys of the palette's rows as shown, top first: `app/instance/view/action`.
fn rows(host: &mut HostTest) -> Vec<String> {
    host.tree()
        .lines()
        .filter_map(|line| {
            let at = line.find(ROW)?;
            let key = line[at + ROW.len()..].split_whitespace().next()?;
            Some(key.to_owned())
        })
        .collect()
}

#[test]
fn the_host_palette_lists_a_text_on_a_canvas_before_the_canvas() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Text");
    host.click("#text.surface");
    host.key("ctrl+shift+p");
    host.wait("#host.palette.search");
    host.run(&["settle"]);
    let listed = rows(&mut host);
    assert!(
        listed.first().is_some_and(|key| key.starts_with(TEXT)),
        "the focused text's actions come first: {listed:?}"
    );

    host.type_text("details");
    host.run(&["settle"]);
    let listed = rows(&mut host);
    let first = |app: &str| listed.iter().position(|key| key.starts_with(app));
    assert!(
        first(TEXT) == Some(0) && first(WORKSPACE) > first(TEXT),
        "searching puts the text's File details above the workspace's: {listed:?}"
    );
    host.snapshot("the_host_palette_lists_a_text_on_a_canvas_before_the_canvas");

    host.key("ctrl+a");
    host.type_text("add a block");
    host.run(&["settle"]);
    let listed = rows(&mut host);
    let add = listed
        .iter()
        .find(|key| key.starts_with(CANVAS) && key.ends_with("/canvas.add.block"))
        .unwrap_or_else(|| panic!("the canvas around the text lists its Add a block: {listed:?}"));
    host.click(&format!("{ROW}{add}"));
    host.gone("#host.palette.search");
    host.wait("#picker.more");
    host.key("Escape");
    host.close();
}
