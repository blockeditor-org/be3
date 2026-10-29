use super::*;
use crate::ForwardedInput;
use crate::KeyAt;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_focused_app_keeps_its_keys_from_global_actions_that_do_not_intercept() {
    let _root = crate::reactive::RootScope::new();
    let forwarded = create_signal(Vec::<Key>::new());
    let ran = create_signal(Vec::new());
    let document = build_node(move || {
        Action::new("test.global", "Global", move || {
            ran.update(|ran| ran.push("global"))
        })
        .shortcut(Chord::logo(Key::G))
        .at(KeyAt::Window)
        .register();
        Action::new("test.intercepting", "Intercepting", move || {
            ran.update(|ran| ran.push("intercepting"))
        })
        .shortcut(Chord::logo(Key::H))
        .at(KeyAt::Window)
        .capture()
        .register();
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                    <Interactive
                        focusable=true
                        on_forward={move |input: ForwardedInput| {
                            if input.focused {
                                forwarded.update(|forwarded| forwarded.extend(input.inputs().filter_map(
                                    |event| match event {
                                        Event::Key { press, .. } if press.pressed && !press.key.is_modifier() => {
                                            Some(press.key)
                                        }
                                        _ => None,
                                    },
                                )));
                            }
                        }}
                    >
                        <Frame />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=200.0 y=0.0 width=100.0 height=100.0>
                    <Interactive focusable=true>
                        <Frame />
                    </Interactive>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(50.0, 50.0));

    harness.key(Key::G, Modifiers::LOGO);
    harness.key(Key::H, Modifiers::LOGO);
    assert_eq!(
        forwarded.get_untracked(),
        [Key::G],
        "the app gets the chord of a global action, but not of one that intercepts"
    );
    assert_eq!(ran.get_untracked(), ["intercepting"]);

    harness.click(pos2(250.0, 50.0));
    harness.key(Key::G, Modifiers::LOGO);
    assert_eq!(
        ran.get_untracked(),
        ["intercepting", "global"],
        "with beui's own control focused, the global action answers"
    );
}
