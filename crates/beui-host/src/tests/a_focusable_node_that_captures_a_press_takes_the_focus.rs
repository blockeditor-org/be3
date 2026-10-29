use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_focusable_node_that_captures_a_press_takes_the_focus() {
    let _root = crate::reactive::RootScope::new();
    let (plain, capturing, surface) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=8.0>
                <unstyled::Button
                    @node_ref=&plain
                    content={move |_: unstyled::ButtonHandle| view! {
                        <Text>"plain"</Text>
                    }}
                />
                <unstyled::Button
                    @node_ref=&capturing
                    capture_presses=true
                    content={move |_: unstyled::ButtonHandle| view! {
                        <Text>"capturing"</Text>
                    }}
                />
                <Interactive @node_ref=&surface capture_presses=true on_press={|_| {}}>
                    <Text>"a surface that takes no focus"</Text>
                </Interactive>
            </List>
        }
    });
    let (plain, capturing, surface) = (plain.get(), capturing.get(), surface.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(plain));
    harness.frame(Vec::new());
    assert_eq!(harness.document().focused_node(), Some(plain));

    harness.click(harness.center(capturing));
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused_node(),
        Some(capturing),
        "a focusable button takes the focus whether or not it captures the press"
    );

    harness.click(harness.center(surface));
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused_node(),
        Some(capturing),
        "a press captured by a node that takes no focus leaves the focus where it was"
    );
}
