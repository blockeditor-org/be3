use super::*;
use crate::ForwardedInput;
use crate::KeyAt;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn an_app_that_inhibits_shortcuts_gets_every_chord_but_the_escape_hatch() {
    let _root = crate::reactive::RootScope::new();
    let (forwarded, set_forwarded) = create_signal(Vec::<Key>::new());
    let (ran, set_ran) = create_signal(Vec::new());
    let (setter, set_setter) = create_signal::<Option<WriteSignal<bool>>>(None);
    let document = build({
        move || {
            Action::new("test.intercepting", "Intercepting", move || {
                set_ran.update(|ran| ran.push("intercepting"))
            })
            .shortcut(Chord::logo(Key::H))
            .at(KeyAt::Window)
            .capture()
            .register();
            Action::new("test.hatch", "Escape hatch", move || {
                set_ran.update(|ran| ran.push("hatch"))
            })
            .shortcut(Chord::logo(Key::L))
            .at(KeyAt::Host)
            .capture()
            .register();
            let (inhibits, set_inhibits) = create_signal(true);
            set_setter.update(|setter| *setter = Some(set_inhibits));
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                        <Interactive
                            focusable=true
                            inhibits_shortcuts={inhibits}
                            on_forward={move |input: ForwardedInput| {
                                if input.focused {
                                    set_forwarded.update(|forwarded| forwarded.extend(input.inputs().filter_map(
                                        |event| match event {
                                            Event::Key { press, .. } if press.pressed => {
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
        }
    });
    let set_inhibits = setter.get_untracked().expect("the document was built");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(50.0, 50.0));

    harness.key(Key::H, Modifiers::LOGO);
    harness.key(Key::F6, Modifiers::NONE);
    harness.key(Key::L, Modifiers::LOGO);
    assert_eq!(
        forwarded.get_untracked(),
        [Key::H, Key::F6],
        "an app that inhibits shortcuts gets intercepted chords and F6, but not the escape hatch"
    );
    assert_eq!(ran.get_untracked(), ["hatch"]);

    set_inhibits.set(false);
    harness.key(Key::H, Modifiers::LOGO);
    assert_eq!(
        ran.get_untracked(),
        ["hatch", "intercepting"],
        "once it lets go, the intercepting action answers again"
    );

    set_inhibits.set(true);
    harness.click(pos2(250.0, 50.0));
    harness.key(Key::H, Modifiers::LOGO);
    assert_eq!(
        ran.get_untracked(),
        ["hatch", "intercepting", "intercepting"],
        "an app that inhibits shortcuts but does not have the focus inhibits nothing"
    );
}
