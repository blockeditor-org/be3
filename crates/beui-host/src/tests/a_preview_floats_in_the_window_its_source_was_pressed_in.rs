use super::*;
use crate::WindowId;
use crate::drag::Carry;
use crate::reactive::*;
use crate::{Window, WindowHandle};

#[test]
fn a_preview_floats_in_the_window_its_source_was_pressed_in() {
    let _root = RootScope::new();
    let window = WindowId::next();
    let document = build_node(move || {
        crate::unstyled::drag_board();
        view! {
            <List spacing=0.0>
                <Frame width=200.0 height=200.0 />
                <Window id={window} size={vec2(160.0, 120.0)}>|_: WindowHandle|
                    <Interactive
                        drag={Func::new(|_: PointerPress| {
                            Some(Carry::new(1_u32).preview(|| view! {
                                <Frame @test_id="preview" width=30.0 height=30.0 />
                            }))
                        })}
                    >
                        <Frame width=40.0 height=40.0 />
                    </Interactive>
                </Window>
            </List>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let button = |pressed| Input::PointerButton {
        pos: pos2(20.0, 20.0),
        button: PointerButton::Primary,
        pressed,
    };
    harness.frame_in(window, vec![Input::PointerMoved(pos2(20.0, 20.0)), button(true)]);
    harness.frame_in(window, vec![Input::PointerMoved(pos2(30.0, 40.0))]);
    harness.frame_in(window, vec![Input::PointerMoved(pos2(40.0, 60.0))]);
    harness.frame_in(window, Vec::new());
    let preview = harness.find("preview");
    assert_eq!(
        harness.document().window_of(preview),
        window,
        "the preview floats in the window its source was pressed in"
    );
}
