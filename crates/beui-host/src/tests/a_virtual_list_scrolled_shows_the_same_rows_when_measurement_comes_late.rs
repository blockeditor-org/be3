use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_virtual_list_scrolled_shows_the_same_rows_when_measurement_comes_late() {
    let (prompt, late) = prompt_and_late(|| {
        let _root = crate::reactive::RootScope::new();
        let document = build_node(move || {
            view! {
                <List spacing=0.0>
                    <Scroll @sizing=ItemSize::Percent(100.0) @test_id="scroll">
                        <VirtualList
                            keys={(0..400).collect::<Vec<usize>>()}
                            item_size=24.0
                        >|index: ReadSignal<usize>|
                            <Text>{create_memo(move || format!("Row {}", index.get()))}</Text>
                        </VirtualList>
                    </Scroll>
                </List>
            }
        });
        let mut harness = Harness::new(document);
        harness.settle();
        let scroll = harness.find("scroll");
        let middle = harness.center(scroll);
        for _ in 0..5 {
            harness.scroll_at(middle, vec2(0.0, -300.0), Modifiers::default());
        }
        harness.settle();
        let mut shown: Vec<(String, Option<Rect>)> = text_rects(&harness)
            .into_iter()
            .filter(|(_, rect)| rect.is_some_and(|rect| rect.intersects(harness.rect(scroll))))
            .collect();
        shown.sort_by(|left, right| left.0.cmp(&right.0));
        shown
    });
    assert!(
        !prompt.iter().any(|(text, _)| text == "Row 0"),
        "the list scrolled: {prompt:?}"
    );
    assert_eq!(
        late, prompt,
        "late measurement settles where prompt measurement does"
    );
}
