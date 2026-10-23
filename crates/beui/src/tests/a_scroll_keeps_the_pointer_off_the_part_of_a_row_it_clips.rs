use super::*;
use crate::geometry::vec2;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_scroll_keeps_the_pointer_off_the_part_of_a_row_it_clips() {
    let _root = crate::reactive::RootScope::new();
    let hovers = create_signal::<Vec<bool>>(Vec::new());
    let clicks = create_signal(0);
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height=100.0 />
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <Interactive
                        on_hover_change={move |hovered: bool| hovers.update(|hovers| hovers.push(hovered))}
                        on_click={move || clicks.update(|clicks| *clicks += 1)}
                    >
                        <Frame height=100.0 />
                    </Interactive>
                    <ForEach keys={indices(20)}>|_: usize|
                        <Frame height=100.0 />
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let x = harness.rect(scroll).center().x;

    harness.scroll_at(pos2(x, 299.0), vec2(0.0, -60.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(scroll), 60.0);

    harness.frame(vec![Input::PointerMoved(pos2(x, 50.0))]);
    assert_eq!(
        hovers.get_untracked(),
        Vec::<bool>::new(),
        "the part of the row scrolled above the scroll is not hovered"
    );
    harness.click(pos2(x, 50.0));
    assert_eq!(clicks.get_untracked(), 0, "nor can it be clicked");

    harness.frame(vec![Input::PointerMoved(pos2(x, 120.0))]);
    assert_eq!(hovers.get_untracked(), vec![true]);

    harness.frame(vec![Input::PointerMoved(pos2(x, 50.0))]);
    assert_eq!(hovers.get_untracked(), vec![true, false]);
}
