use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::styled::Reorder;
use crate::unstyled::{
    Choice, ChoiceItem, ChoiceKind, ChoiceOption, ChoiceOptionHandle, ReorderHandle, ReorderItem,
    ReorderItemHandle, ReorderMove,
};

/// The dock's tab strip, in small: a `Choice` of tabs whose items are `ReorderItem`s, handing
/// the `Reorder` its rows through `handle`, as a `ForEach` would.
#[test]
fn a_choice_hands_a_reorder_its_tabs_so_a_stepped_tab_glides() {
    let _root = RootScope::new();
    let keys = create_signal(vec![0_usize, 1, 2]);
    let held = StoredCell::new(None::<ReorderHandle<usize, usize>>);
    let document = build_node(move || {
        view! {
            <Reorder
                value={keys}
                direction=Direction::Horizontal
                payload={Func::new(|key: usize| Some(key))}
                preview={|key: usize| view! {
                    <Frame width={TAB_WIDTHS[key]} height=24.0 />
                }}
                on_move={move |moved: ReorderMove<usize, usize>| keys.update(|keys| {
                    keys.retain(|key| *key != moved.payload);
                    keys.insert(moved.index, moved.payload);
                })}
            >|reorder: ReorderHandle<usize, usize>| {
                held.set(Some(reorder));
                let order = reorder.order;
                view! {
                    <Choice
                        options={view! {
                            <ForEach value={order}>|key: usize|
                                <ChoiceOption value=key label={format!("Tab {key}")} />
                            </ForEach>
                        }}
                        item={move |item: ChoiceItem<usize>| {
                            let ChoiceItem { value, button } = item;
                            view! {
                                <ReorderItem reorder item=value>|_: ReorderItemHandle| {
                                    button
                                }</ReorderItem>
                            }
                        }}
                        handle={reorder.rows}
                        kind=ChoiceKind::Tabs
                        spacing=4.0
                    >|tab: ChoiceOptionHandle<usize>|
                        <Frame
                            @test_id={format!("tab.{}", tab.value)}
                            width={TAB_WIDTHS[tab.value]}
                            height=24.0
                        />
                    </Choice>
                }
            }</Reorder>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let reorder = held.get().expect("the reorder handed its handle");
    assert_eq!(
        reorder.rows.nodes_of(&1).len(),
        1,
        "the choice reports the item each tab built"
    );
    let was = tab_left(&harness, 2);

    assert!(reorder.step(2, -2), "a step moves the last tab first");
    harness.frame(Vec::new());
    assert_eq!(keys.get_untracked(), [2, 0, 1]);
    assert_eq!(
        tab_left(&harness, 2),
        was,
        "the first frame after the step paints the tab where it was"
    );
    harness.settle();
    assert_eq!(tab_left(&harness, 2), 0.0, "and it glides to its new place");
}
