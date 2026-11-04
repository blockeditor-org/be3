use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_press_outside_an_interactive_is_reported_to_it() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::<Pos2>::new());
    let watcher = NodeRef::new();
    let elsewhere = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Interactive
                    @node_ref={&watcher}
                    on_press_outside={move |pos: Pos2| heard.update(|heard| heard.push(pos))}
                >
                    <Frame height=100.0 />
                </Interactive>
                <Interactive @node_ref={&elsewhere} on_click={|| {}}>
                    <Frame height=100.0 />
                </Interactive>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.rect(watcher.get()).center());
    assert!(
        heard.with_untracked(|heard| heard.is_empty()),
        "a press inside is not outside: {:?}",
        heard.get_untracked()
    );

    let outside = harness.rect(elsewhere.get()).center();
    harness.click(outside);
    assert_eq!(
        heard.get_untracked(),
        vec![outside],
        "a press on something else that takes it is still heard, once"
    );

    harness.touch(TouchPhase::Start, outside);
    harness.touch(TouchPhase::End, outside);
    assert_eq!(
        heard.with_untracked(|heard| heard.len()),
        2,
        "a finger press is a press too"
    );
}
