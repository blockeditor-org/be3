use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;
use std::cell::RefCell;

const ROW: f32 = 24.0;
const ANCHOR: usize = 30;

/// A scroll that moves its offset to keep a list's anchor row in place lays its content out again
/// in the same frame. It reports its position once, from that layout, with the offset after the
/// shift; a report from the layout that shifted would pair the old offset with rows the shift had
/// yet to move.
#[test]
fn a_scroll_that_shifts_to_keep_its_anchor_reports_once_from_where_it_ends() {
    let _root = crate::reactive::RootScope::new();
    let reports: Rc<RefCell<Vec<f32>>> = Rc::default();
    let heard = Rc::clone(&reports);
    let grown = Rc::new(std::cell::Cell::new(None));
    let handle = Rc::clone(&grown);
    let document = build_node(move || {
        let tall = create_signal(false);
        handle.set(Some(tall));
        let heard = Rc::clone(&heard);
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Scroll
                        @test_id="scroll"
                        style={plain_scroll()}
                        offset={ROW * ANCHOR as f32}
                        on_change={move |position: ScrollPosition| {
                            heard.borrow_mut().push(position.offset)
                        }}
                    >
                        <VirtualList
                            value={(0..200).collect::<Vec<usize>>()}
                            item_size=ROW
                            overscan={ROW * 8.0}
                            anchor={Some(ANCHOR)}
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
    let tall = grown.get().expect("the list was built");
    let mut harness = Harness::new(document);
    harness.settle();
    reports.borrow_mut().clear();

    tall.set(true);
    harness.settle();
    let reported = reports.borrow().clone();
    let [offset] = reported[..] else {
        panic!("one report after the rows above the anchor grew: {reported:?}");
    };
    assert!(
        offset > ROW * ANCHOR as f32 + 5.0 * ROW,
        "the report is the offset after the shift, not the one before it: {offset}"
    );
    let scroll = harness.rect(harness.find("scroll")).min.y;
    let row = harness.rect(harness.find(&format!("row.{ANCHOR}"))).min.y;
    assert!(
        (row - scroll).abs() < 0.5,
        "where row {ANCHOR} is still at the top: {row} against {scroll}"
    );
}
