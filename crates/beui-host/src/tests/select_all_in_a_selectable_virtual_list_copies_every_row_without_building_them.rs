use super::*;
use crate::primitives::{Text, VirtualList};
use crate::reactive::*;
use crate::styled::{Scroll, SelectableText};

const ROWS: usize = 1000;

fn label(index: usize) -> String {
    format!("Line {index}")
}

#[component]
fn Line(index: ReadSignal<usize>, built: Signal<usize>) -> NodeId {
    built.update(|built| *built += 1);
    let text = create_memo(move || label(index.get()));
    view! {
        <Text>{text}</Text>
    }
}

#[test]
fn select_all_in_a_selectable_virtual_list_copies_every_row_without_building_them() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let built = create_signal(0_usize);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&scroll @sizing=ItemSize::Fixed(100.0)>
                    <SelectableText>
                        <VirtualList
                            keys={(0..ROWS).collect::<Vec<usize>>()}
                            item_size=20.0
                            text={label}
                        >|index: ReadSignal<usize>|
                            <Line index built />
                        </VirtualList>
                    </SelectableText>
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let area = harness.rect(scroll.get());
    harness.click(pos2(area.left() + 1.0, area.top() + 8.0));
    harness.frame(Vec::new());
    let before = built.get_untracked();

    harness.frame(pressed_with(Key::A, Modifiers::CTRL));
    let copied = harness
        .frame(pressed_with(Key::C, Modifiers::CTRL))
        .copied_text
        .expect("Ctrl+C copies the selection");
    let expected = (0..ROWS).map(label).collect::<Vec<_>>().join("\n");
    assert_eq!(copied, expected, "the copy holds every row, one per line");
    assert_eq!(
        built.get_untracked(),
        before,
        "selecting and copying builds no rows"
    );
}
