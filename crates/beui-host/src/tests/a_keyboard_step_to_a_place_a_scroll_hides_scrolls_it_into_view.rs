use super::*;
use crate::drag::{self, Carry};
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::unstyled::{place_on, use_drop_site};

const ROWS: usize = 20;
const ROW: f32 = 30.0;

#[test]
fn a_keyboard_step_to_a_place_a_scroll_hides_scrolls_it_into_view() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let over = create_signal(None::<usize>);
    let rows: Vec<NodeRef> = (0..ROWS).map(|_| NodeRef::new()).collect();
    let document = {
        let rows = rows.clone();
        build_node(move || {
            let places = rows.clone();
            let site = use_drop_site(
                Func::new(|_: u32| true),
                Func::new(move |_: u32| {
                    drag::closest(
                        places
                            .iter()
                            .enumerate()
                            .filter_map(|(at, row)| place_on(*row, at)),
                    )
                }),
                Callback::new(|_: (u32, usize)| {}),
            );
            create_effect(move || over.set(site.over.get()));
            let rows = rows.clone();
            view! {
                <List spacing=0.0>
                    <Interactive
                        focusable=true
                        drag={Func::new(|_: PointerPress| Some(Carry::new(7_u32)))}
                    >
                        <Frame width=40.0 height=40.0 />
                    </Interactive>
                    <Frame height=100.0>
                        <Scroll @node_ref=&scroll>
                            <List spacing=0.0>
                                <ForEach value={(0..ROWS).collect::<Vec<usize>>()}>|at: usize| {
                                    let row = rows[at];
                                    view! {
                                        <Frame @node_ref=&row height=ROW />
                                    }
                                }</ForEach>
                            </List>
                        </Scroll>
                    </Frame>
                </List>
            }
        })
    };
    let mut harness = Harness::new(document);
    harness.settle();
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    harness.settle();
    for _ in 0..12 {
        harness.key(Key::ArrowDown, Modifiers::NONE);
        harness.settle();
    }

    let chosen = over.get_untracked().expect("the carry is over a row");
    assert!(chosen >= 10, "the steps went down the rows, to {chosen}");
    let viewport = harness.rect(scroll.get());
    let row = harness.rect(rows[chosen].get());
    assert!(
        harness.document().scroll_offset(scroll.get()) > 0.0,
        "the scroll moved to show it"
    );
    assert!(
        row.top() >= viewport.top() - 0.5 && row.bottom() <= viewport.bottom() + 0.5,
        "the row the step chose shows: {row:?} in {viewport:?}"
    );
}
