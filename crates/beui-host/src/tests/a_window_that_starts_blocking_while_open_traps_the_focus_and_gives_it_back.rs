use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_window_that_starts_blocking_while_open_traps_the_focus_and_gives_it_back() {
    let _root = crate::reactive::RootScope::new();
    let (opener, inside) = (NodeRef::new(), NodeRef::new());
    let open = create_signal(false);
    let blocks = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=8.0>
                <Button @node_ref=&opener label="Open" on_click={move || open.set(true)} />
                <Window
                    size={vec2(200.0, 100.0)}
                    own_window=false
                    open={open}
                    blocks={blocks}
                    at={pos2(200.0, 200.0)}
                >|_: WindowHandle|
                    <Button @node_ref=&inside label="Inside" focused={open} />
                </Window>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(opener.get()));
    harness.frame(Vec::new());
    assert_eq!(harness.document().focused, Some(inside.get()));

    blocks.set(true);
    harness.frame(Vec::new());
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused,
        Some(inside.get()),
        "once it blocks, Tab stays inside it"
    );

    open.set(false);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused,
        Some(opener.get()),
        "and closing gives the focus back to what had it before it opened"
    );
}
