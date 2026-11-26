use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

const ROW: f32 = 24.0;
const ANCHOR: usize = 30;

/// The top of row `ANCHOR`, and of the scroll, after the rows just above it grow (a code block
/// measured anew) while `anchor` names it or nothing does.
fn anchor_and_scroll_tops(anchor: Option<usize>) -> (f32, f32) {
    let _root = crate::reactive::RootScope::new();
    let heights = Rc::new(std::cell::Cell::new(None));
    let handle = Rc::clone(&heights);
    let document = build_node(move || {
        let tall = create_signal(false);
        handle.set(Some(tall));
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Scroll @test_id="scroll" style={plain_scroll()} offset={ROW * ANCHOR as f32}>
                        <VirtualList
                            value={(0..200).collect::<Vec<usize>>()}
                            item_size=ROW
                            overscan={ROW * 8.0}
                            anchor={anchor}
                        >|index: ReadSignal<usize>|
                            <Frame
                                @test_id={create_memo(move || format!("row.{}", index.get()))}
                                height={create_memo(move || {
                                    let grows = (ANCHOR - 5..ANCHOR).contains(&index.get());
                                    match tall.get() && grows {
                                        true => ROW * 3.0,
                                        false => ROW,
                                    }
                                })}
                            />
                        </VirtualList>
                    </Scroll>
                </ListItem>
            </List>
        }
    });
    let tall = heights.get().expect("the list was built");
    let mut harness = Harness::new(document);
    harness.settle();
    let scroll = harness.rect(harness.find("scroll")).min.y;
    let before = harness.rect(harness.find(&format!("row.{ANCHOR}"))).min.y;
    assert!(
        (before - scroll).abs() < 0.5,
        "row {ANCHOR} starts at the top: {before} against {scroll}"
    );
    tall.set(true);
    harness.settle();
    (
        harness.rect(harness.find(&format!("row.{ANCHOR}"))).min.y,
        scroll,
    )
}

#[test]
fn a_virtual_lists_anchor_row_stays_put_as_the_rows_above_it_grow() {
    let (anchored, scroll) = anchor_and_scroll_tops(Some(ANCHOR));
    assert!(
        (anchored - scroll).abs() < 0.5,
        "the named row stays at the top as the rows above it grow: {anchored} against {scroll}"
    );
    let (unanchored, _) = anchor_and_scroll_tops(None);
    assert!(
        (unanchored - scroll).abs() > ROW,
        "with no row named, the first row built stays put and the growth pushes the rest down: \
         {unanchored}"
    );
}
