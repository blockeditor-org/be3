use super::*;
use crate::drag::{Candidate, Carry, closest};
use crate::reactive::*;
use crate::styled::CommandPalette;
use crate::unstyled::{CommandPaletteApi, carrying, use_drop_site};
use crate::{KeyEvent, KeyPhase};

/// A tab-like source: it activates on Space and Enter as a button does, and moves between
/// three baskets with Ctrl+Shift and an arrow, one place at once, or by "Move" in the command
/// palette, which starts a whole keyboard carry.
#[test]
fn a_source_that_activates_keeps_its_keys_and_moves_with_ctrl_shift_or_the_palette() {
    let _root = RootScope::new();
    let dropped = create_signal(Vec::<&'static str>::new());
    let activated = create_signal(0_u32);
    let carried = create_signal(None::<u32>);
    let palette = NodeRef::new();
    let document = build_node(move || {
        let baskets = [NodeRef::new(), NodeRef::new(), NodeRef::new()];
        let names = ["left", "middle", "right"];
        use_drop_site(
            Func::new(|_: u32| true),
            Func::new(move |_: u32| {
                closest(baskets.iter().zip(names).map(|(basket, name)| {
                    Candidate::new(
                        name,
                        Rect::from_min_size(Pos2::ZERO, vec2(60.0, 40.0)),
                        *basket,
                    )
                }))
            }),
            Callback::new(move |(_, name): (u32, &'static str)| {
                dropped.update(|dropped| dropped.push(name))
            }),
        );
        create_effect(move || carried.set(carrying::<u32>()));
        let open = create_signal(false);
        on_key(
            KeyAt::Root,
            KeyPhase::Capture,
            KeyReveal::Focus,
            move |event: KeyEvent| {
                let wanted = event.press().is_some_and(|press| {
                    press.pressed
                        && press.key == Key::P
                        && press.modifiers.ctrl
                        && press.modifiers.shift
                });
                if wanted {
                    open.set(true);
                }
                wanted
            },
        );
        view! {
            <List spacing=20.0 align=Align::Start>
                <Interactive
                    focusable=true
                    on_activate={move || activated.update(|count| *count += 1)}
                    drag={Func::new(|_: PointerPress| Some(Carry::new(7_u32)))}
                >
                    <Frame width=60.0 height=40.0 />
                </Interactive>
                <List direction=Direction::Horizontal spacing=30.0>
                    <Frame @node_ref={&baskets[0]} width=60.0 height=40.0 />
                    <Frame @node_ref={&baskets[1]} width=60.0 height=40.0 />
                    <Frame @node_ref={&baskets[2]} width=60.0 height=40.0 />
                </List>
                <CommandPalette @node_ref=&palette open />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    harness.key(Key::Tab, Modifiers::NONE);

    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    assert_eq!(
        carried.get_untracked(),
        None,
        "Space does not lift a source that activates"
    );
    assert_eq!(
        activated.get_untracked(),
        2,
        "Space and Enter activate it, as a button"
    );

    let ctrl_shift = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };
    harness.key(Key::ArrowUp, ctrl_shift);
    assert!(
        dropped.get_untracked().is_empty(),
        "with no place that way, Ctrl+Shift and an arrow drop nothing"
    );
    assert_eq!(carried.get_untracked(), None, "and leave nothing carried");
    harness.key(Key::ArrowRight, ctrl_shift);
    assert_eq!(
        dropped.get_untracked(),
        ["middle"],
        "Ctrl+Shift and an arrow move it one place that way at once"
    );
    assert_eq!(carried.get_untracked(), None);

    harness.key(Key::P, ctrl_shift);
    harness.type_text("Move");
    assert_eq!(
        harness
            .document()
            .api::<CommandPaletteApi>(palette.get())
            .shown(),
        vec!["drag.move"],
        "the palette offers Move while the source has the focus"
    );
    harness.key(Key::Enter, Modifiers::NONE);
    harness.settle();
    assert_eq!(
        carried.get_untracked(),
        Some(7),
        "Move lifts it with the keyboard"
    );
    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    assert_eq!(dropped.get_untracked(), ["middle", "right"]);
    assert_eq!(
        activated.get_untracked(),
        2,
        "and carrying activates nothing"
    );
}
