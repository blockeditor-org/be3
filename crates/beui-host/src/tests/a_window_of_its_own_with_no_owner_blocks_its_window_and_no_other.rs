use super::*;

#[test]
fn a_window_of_its_own_with_no_owner_blocks_its_window_and_no_other() {
    let _root = crate::reactive::RootScope::new();
    let main_pressed = create_signal(0_u32);
    let other_pressed = create_signal(0_u32);
    let (blocking, other) = (WindowId::next(), WindowId::next());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Interactive on_press={move |_: PointerPress| main_pressed.update(|n| *n += 1)}>
                    <Frame width=600.0 height=400.0 />
                </Interactive>
                <Window id={other} size={vec2(200.0, 120.0)}>|_: WindowHandle|
                    <Interactive
                        on_press={move |_: PointerPress| other_pressed.update(|n| *n += 1)}
                    >
                        <Frame />
                    </Interactive>
                </Window>
                <Window
                    id={blocking}
                    size={vec2(200.0, 120.0)}
                    anchor={Some(WindowAnchor::Centre)}
                    blocks=true
                >|_: WindowHandle|
                    <Frame />
                </Window>
            </List>
        }
    });
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    harness.click(pos2(50.0, 300.0));
    harness.frame(Vec::new());
    assert_eq!(
        main_pressed.get_untracked(),
        0,
        "with no owner it blocks its whole window"
    );
    press_in(&mut harness, other, pos2(20.0, 20.0), true);
    press_in(&mut harness, other, pos2(20.0, 20.0), false);
    assert_eq!(
        other_pressed.get_untracked(),
        1,
        "and no other window of its own: they are not its owner's"
    );
}
