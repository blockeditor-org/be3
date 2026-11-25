use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

const PANE: &str = "Pane \"Checklist\"";

/// The card holding a block on a canvas.
const CARD: &str = "GenericContainer #infinite-canvas.entity.";

/// The places of every line of `tree` naming `target`, in its order.
fn places(tree: &str, target: &str) -> Vec<(f32, f32)> {
    tree.lines()
        .filter(|line| line.contains(target))
        .filter_map(|line| {
            let numbers: Vec<f32> = line
                .split(" at ")
                .nth(1)?
                .split(|character: char| !character.is_ascii_digit() && character != '.')
                .filter_map(|number| number.parse().ok())
                .collect();
            Some((*numbers.first()?, *numbers.get(1)?))
        })
        .collect()
}

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_a_block_placed_twice_with_an_editor_in_each() {
    // be-host's own UI puts a checklist on a canvas and duplicates its card, which places the
    // same block twice; started again with the root in place of its UI, each place shows a
    // live editor of its own, as two views of one open block each get their own: a filter
    // chosen in the second leaves the first on All, and a task added from the second's own
    // draft reaches the first through the block they both edit.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    host.run(&["resize", "1700x900"]);
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Checklist");
    host.wait(PANE);
    host.wait("#checklist.add");
    let canvas = host.place("#infinite-canvas.canvas");
    let empty = format!("{:.0},{:.0}", canvas.0 + 20.0, canvas.1 + canvas.3 - 20.0);
    let card = host.place(CARD);
    let middle = |at: (f32, f32, f32, f32)| (at.0 + at.2 / 2.0, at.1 + at.3 / 2.0);
    let draft = middle(host.place("#checklist.draft"));
    let add = middle(host.place("#checklist.add"));
    let open = middle(host.place("#checklist.filter.open"));
    host.click(&empty);
    host.key("ctrl+a");
    host.key("ctrl+d");
    host.run(&["settle"]);
    // The duplicate is placed over the card, 24 points down and right: drag it aside by its
    // title bar, which is the canvas's.
    let title = (card.0 + card.2 / 2.0 + 24.0, card.1 + 24.0 + 25.0);
    let from = format!("{:.0},{:.0}", title.0, title.1);
    let to = format!("{:.0},{:.0}", title.0 + 470.0, title.1 + 40.0);
    host.run(&["drag", &from, &to]);
    host.run(&["settle"]);
    // The second card's controls, where the first's are, moved as far as the card was.
    let cards = places(&host.tree(), CARD);
    let [first, second] = cards[..] else {
        panic!("the canvas holds two cards, not {cards:?}");
    };
    let in_second = |at: (f32, f32)| {
        format!(
            "{:.0},{:.0}",
            at.0 + second.0 - first.0,
            at.1 + second.1 - first.1
        )
    };
    host.restart_as_root(ROOT);
    host.run(&["resize", "1700x900"]);
    host.run(&["settle"]);
    host.snapshot("be_root_shows_a_block_placed_twice_with_an_editor_in_each");
    // The second place's filter set to Open: the first, an editor of its own, stays on All.
    host.run(&["click", &in_second(open)]);
    host.run(&["settle"]);
    host.snapshot("be_root_filters_one_place_of_a_block_placed_twice");
    // A task added from the second place's own draft shows in the first too: both edit the
    // one block.
    host.run(&["click", &in_second(draft)]);
    host.type_text("buy milk");
    host.run(&["click", &in_second(add)]);
    host.run(&["settle"]);
    host.snapshot("be_root_shows_a_task_added_in_one_place_in_both");
    host.close();
}
