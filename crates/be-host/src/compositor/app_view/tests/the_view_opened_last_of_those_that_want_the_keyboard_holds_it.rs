use super::*;
use beui::native::Context;
use beui::native::WindowSize;
use beui::vec2;
use beui_host::native::FreetypeFonts;
use beui_host::native::build;
use beui_host::native::testing::{Showing, Stepped};

#[test]
fn the_view_opened_last_of_those_that_want_the_keyboard_holds_it() {
    let _root = RootScope::new();
    let older = NodeRef::new();
    let newer = NodeRef::new();
    let wants = [create_signal(false), create_signal(false)];
    let document = build(move || {
        for ((catcher, wants), mount) in [older, newer].iter().zip(wants).zip([1, 2]) {
            let state = create_memo(move || PlacedView {
                wants_keyboard: wants.get(),
                ..placed(Vec::new())
            });
            take_keyboard(state, None, catcher, create_memo(|| true), mount);
        }
        view! {
            <beui::styled::Root>
                <Layers>
                    <Interactive
                        @node_ref={&older}
                        focusable=true
                        on_forward={|_: ForwardedInput| {}}
                    />
                    <Interactive
                        @node_ref={&newer}
                        focusable=true
                        on_forward={|_: ForwardedInput| {}}
                    />
                </Layers>
            </beui::styled::Root>
        }
    });
    let mut stepped = Stepped::new(
        Context::new(FreetypeFonts::default()),
        Showing(document),
        WindowSize {
            size: vec2(400.0, 300.0),
            scale: 1.0,
        },
        None,
    );
    stepped.frame(Vec::new());
    let mut focused_after = |wanting: [bool; 2]| {
        for (signal, wanted) in wants.iter().zip(wanting) {
            signal.set(wanted);
        }
        stepped.frame(Vec::new());
        let focused = stepped.document().focused_node();
        match focused {
            _ if focused == older.try_get() => "older",
            _ if focused == newer.try_get() => "newer",
            _ => "neither",
        }
    };

    assert_eq!(focused_after([false, true]), "newer");
    assert_eq!(
        focused_after([true, true]),
        "newer",
        "a view opened earlier that comes to want the keyboard leaves it with the one opened later"
    );
    assert_eq!(
        focused_after([true, false]),
        "older",
        "and takes it when that one lets it go"
    );
    assert_eq!(
        focused_after([true, true]),
        "newer",
        "the view opened later takes it back"
    );
}
