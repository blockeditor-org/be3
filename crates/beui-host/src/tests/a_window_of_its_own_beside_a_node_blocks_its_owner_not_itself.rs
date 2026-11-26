use super::*;

#[test]
fn a_window_of_its_own_beside_a_node_blocks_its_owner_not_itself() {
    let _root = crate::reactive::RootScope::new();
    let (owner, control, inside) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let owned = create_signal(None::<NodeId>);
    let owner_pressed = create_signal(0_u32);
    let inside_pressed = create_signal(0_u32);
    let window = WindowId::next();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Interactive
                    @node_ref=&owner
                    on_press={move |_: PointerPress| owner_pressed.update(|n| *n += 1)}
                >
                    <List spacing=0.0>
                        <Frame @node_ref=&control width=80.0 height=20.0 />
                        <Frame width=600.0 height=380.0 />
                    </List>
                </Interactive>
                <Window
                    id={window}
                    size={vec2(200.0, 120.0)}
                    owner={owned}
                    anchor={Some(WindowAnchor::Node(control))}
                    blocks=true
                    scrim={SCRIM}
                >|_: WindowHandle|
                    <Interactive
                        @node_ref=&inside
                        on_press={move |_: PointerPress| inside_pressed.update(|n| *n += 1)}
                    >
                        <Frame />
                    </Interactive>
                </Window>
            </List>
        }
    });
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    owned.set(Some(owner.get()));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());
    let dimmed = output.main().shapes().iter().any(|shape| {
        matches!(shape, Shape::Rect { rect, color, .. } if *color == SCRIM && rect.contains(pos2(50.0, 300.0)))
    });
    assert!(dimmed, "beside a node, the owner is what it dims");
    harness.click(pos2(50.0, 300.0));
    harness.frame(Vec::new());
    assert_eq!(owner_pressed.get_untracked(), 0, "and what it shuts out");
    press_in(&mut harness, window, pos2(20.0, 20.0), true);
    press_in(&mut harness, window, pos2(20.0, 20.0), false);
    assert_eq!(
        inside_pressed.get_untracked(),
        1,
        "while its own content takes presses"
    );
}
