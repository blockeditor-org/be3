use super::*;
use crate::drag::{Candidate, Carry, closest};
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::use_drop_site;

/// Enter lifts a focused source's carry as Space does, once the source's own keys have
/// turned it down: a source whose `on_key` takes Enter keeps it.
#[test]
fn enter_lifts_a_source_whose_own_keys_turn_it_down() {
    let _root = RootScope::new();
    let dropped = create_signal(Vec::<u32>::new());
    let entered = create_signal(0);
    let document = build_node(move || {
        let basket = NodeRef::new();
        use_drop_site(
            Func::new(|_: u32| true),
            Func::new(move |_: u32| {
                closest([Candidate::new(
                    "basket",
                    Rect::from_min_size(Pos2::ZERO, vec2(60.0, 40.0)),
                    basket,
                )])
            }),
            Callback::new(move |(carried, _): (u32, &'static str)| {
                dropped.update(|dropped| dropped.push(carried))
            }),
        );
        view! {
            <List spacing=20.0 align=Align::Start>
                <Interactive
                    focusable=true
                    drag={Func::new(|_: PointerPress| Some(Carry::new(1_u32)))}
                >
                    <Frame width=60.0 height=40.0 />
                </Interactive>
                <Interactive
                    focusable=true
                    on_key={move |press: KeyPress| {
                        if press.key != Key::Enter {
                            return false;
                        }
                        if press.pressed {
                            entered.update(|entered| *entered += 1);
                        }
                        true
                    }}
                    drag={Func::new(|_: PointerPress| Some(Carry::new(2_u32)))}
                >
                    <Frame width=60.0 height=40.0 />
                </Interactive>
                <Frame @node_ref={&basket} width=60.0 height=40.0 />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    assert_eq!(dropped.get_untracked(), [1], "Enter lifts it, and Enter drops it");

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    assert_eq!(entered.get_untracked(), 2, "the source's own Enter is its own");
    assert_eq!(dropped.get_untracked(), [1], "and lifts nothing");
}
