use super::*;
use crate::primitives::{Text, VirtualList};
use crate::reactive::*;
use crate::styled::{Scroll, SelectableText};

const ROWS: usize = 200;

fn label(index: usize) -> String {
    format!("Row {index}")
}

#[component]
fn Line(index: ReadSignal<usize>) -> NodeId {
    let text = create_memo(move || label(index.get()));
    view! {
        <Text>{text}</Text>
    }
}

#[test]
fn dragging_past_the_bottom_of_a_selectable_virtual_list_scrolls_it_and_selects_rows_out_of_view() {
    let _root = crate::reactive::RootScope::new();
    let (region, scroll) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&scroll @sizing=ItemSize::Fixed(100.0)>
                    <SelectableText @node_ref=&region>
                        <VirtualList
                            keys={(0..ROWS).collect::<Vec<usize>>()}
                            item_size=20.0
                            text={label}
                        >|index: ReadSignal<usize>|
                            <Line index />
                        </VirtualList>
                    </SelectableText>
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let area = harness.rect(scroll.get());
    let selected = |harness: &mut Harness| selected_text(harness, region.get());

    harness.press_at(pos2(area.left() + 1.0, area.top() + 8.0));
    harness.hover(pos2(area.left() + 40.0, area.bottom() + 40.0));
    for _ in 0..30 {
        harness.frame(Vec::new());
    }
    harness.release_at(pos2(area.left() + 40.0, area.bottom() + 40.0));
    harness.frame(Vec::new());

    let offset = harness.document().scroll_offset(scroll.get());
    assert!(
        offset > 200.0,
        "holding the pointer below the list scrolls it: {offset}"
    );
    let copied = selected(&mut harness);
    let lines: Vec<&str> = copied.lines().collect();
    assert!(
        lines.len() > 12,
        "the selection grows with the scroll: {copied:?}"
    );
    let expected: Vec<String> = (0..lines.len()).map(label).collect();
    assert_eq!(
        lines[..lines.len() - 1],
        expected[..lines.len() - 1],
        "rows scrolled out of view copy from the list's text"
    );

    for _ in 0..20 {
        harness.scroll_at(area.center(), vec2(0.0, 400.0), Modifiers::NONE);
    }
    let output = harness.frame(Vec::new());
    let highlighted = output
        .main()
        .shapes()
        .iter()
        .filter(|shape| {
            matches!(shape, crate::native::Shape::Rect { color, stroke_width, .. }
                if *color == styled::Theme::DARK.accent_soft && *stroke_width == 0.0)
        })
        .count();
    assert!(
        highlighted >= 4,
        "rows built again on the way back up paint the selection: {highlighted}"
    );
    assert_eq!(
        selected(&mut harness),
        copied,
        "scrolling keeps the selection"
    );
}
