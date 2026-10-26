use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

const TITLE: &str = "Fix the launcher";

#[test]
fn dragging_across_texts_selects_them_and_ctrl_c_copies_the_selection() {
    let _root = crate::reactive::RootScope::new();
    let (title, labels, region) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <SelectableText @node_ref=&region>
                <List spacing=4.0>
                    <Text @node_ref=&title>{TITLE}</Text>
                    <List direction=Direction::Horizontal spacing=8.0>
                        <Text>"#210 by someone"</Text>
                        <Text @node_ref=&labels>"ready"</Text>
                    </List>
                </List>
            </SelectableText>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (title, labels) = (harness.rect(title.get()), harness.rect(labels.get()));
    let copy = |harness: &mut Harness| {
        harness
            .frame(pressed_with(Key::C, Modifiers::CTRL))
            .copied_text
    };

    harness.drag(
        pos2(title.left() + 1.0, title.center().y),
        pos2(labels.right() + 20.0, labels.center().y),
    );
    assert_eq!(
        selected_text(&mut harness, region.get()),
        "Fix the launcher\n#210 by someone ready"
    );
    assert_eq!(
        copy(&mut harness).as_deref(),
        Some("Fix the launcher\n#210 by someone ready"),
        "texts on one line are joined by a space and lines by a line break"
    );

    harness.drag(
        pos2(title.left() + 30.0, title.center().y),
        pos2(labels.right() + 20.0, labels.center().y),
    );
    let partial = copy(&mut harness).expect("the second drag selects too");
    let (start, rest) = partial.split_once('\n').expect("two lines are selected");
    assert!(
        !start.is_empty() && start.len() < TITLE.len() && TITLE.ends_with(start),
        "a drag from inside a text selects from where it started: {partial:?}"
    );
    assert_eq!(rest, "#210 by someone ready");

    harness.advance(Duration::from_secs(1));
    harness.click(pos2(title.left() + 30.0, title.center().y));
    assert_eq!(copy(&mut harness), None, "a click clears the selection");
}
