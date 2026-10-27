use super::*;
use crate::input::{LONG_PRESS_DELAY, Pen, PenPhase, PointerPress, ZoomGesture};
use crate::primitives::*;
use crate::reactive::*;
use crate::{CanvasTouch, PreferenceLayer, PreferenceSource};

#[test]
fn fingers_pass_over_a_drawing_surface_while_canvas_touch_is_off() {
    let _root = crate::reactive::RootScope::new();
    let drawn = create_signal::<Vec<bool>>(Vec::new());
    let menus = create_signal(0);
    let zoom = create_signal(1.0_f32);
    let tapped = create_signal(0);
    let surface = NodeRef::new();
    let button = NodeRef::new();
    let document = build_node(move || {
        let zoomed =
            crate::unstyled::gestures::pan_zoom(Callback::empty(), move |gesture: ZoomGesture| {
                zoom.set(zoom.get_untracked() * gesture.factor)
            });
        view! {
            <List spacing=0.0>
                <Interactive on_pointer={zoomed.pointer} on_wheel={zoomed.wheel}>
                    <Interactive
                        @node_ref=&surface
                        drawing=true
                        touch_drags=true
                        on_press={move |press: PointerPress| drawn.update(|drawn| drawn.push(press.touch))}
                        on_secondary_press={move |_: PointerPress| menus.update(|menus| *menus += 1)}
                    >
                        <Frame width=300.0 height=300.0 />
                    </Interactive>
                </Interactive>
                <Interactive
                    @node_ref=&button
                    on_click={move || tapped.update(|tapped| *tapped += 1)}
                >
                    <Frame width=300.0 height=60.0 />
                </Interactive>
            </List>
        }
    });
    let (surface, button) = (surface.get(), button.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let center = harness.rect(surface).center();

    harness.touch(TouchPhase::Start, center);
    harness.touch(TouchPhase::End, center);
    assert_eq!(
        drawn.get_untracked(),
        vec![true],
        "a finger draws by default"
    );

    harness.context().set_preference_layer(
        PreferenceSource::App,
        PreferenceLayer {
            canvas_touch: Some(CanvasTouch::Off),
            ..PreferenceLayer::EMPTY
        },
    );
    harness.frame(Vec::new());
    harness.touch(TouchPhase::Start, center);
    harness.touch(TouchPhase::Move, center + vec2(40.0, 0.0));
    harness.advance(LONG_PRESS_DELAY * 2);
    harness.frame(Vec::new());
    harness.touch(TouchPhase::End, center + vec2(40.0, 0.0));
    harness.touch(TouchPhase::Start, center);
    harness.advance(LONG_PRESS_DELAY * 2);
    harness.frame(Vec::new());
    harness.touch(TouchPhase::End, center);
    assert_eq!(
        drawn.get_untracked(),
        vec![true],
        "a resting finger neither draws nor drags"
    );
    assert_eq!(menus.get_untracked(), 0, "a held finger opens no menu");

    harness.fingers(
        TouchPhase::Start,
        &[(1, center - vec2(50.0, 0.0)), (2, center + vec2(50.0, 0.0))],
    );
    harness.move_fingers(center - vec2(100.0, 0.0), center + vec2(100.0, 0.0));
    harness.fingers(
        TouchPhase::End,
        &[
            (1, center - vec2(100.0, 0.0)),
            (2, center + vec2(100.0, 0.0)),
        ],
    );
    assert_eq!(zoom.get_untracked(), 1.0, "two fingers do not zoom it");

    let pen = |phase| Input::Pen {
        phase,
        pos: center,
        pen: Pen {
            pressure: 0.5,
            ..Pen::default()
        },
    };
    harness.frame(vec![pen(PenPhase::Down(PointerButton::Primary))]);
    harness.frame(vec![pen(PenPhase::Up(PointerButton::Primary))]);
    harness.click(center);
    assert_eq!(
        drawn.get_untracked(),
        vec![true, false, false],
        "a pen and a mouse still draw"
    );

    let below = harness.rect(button).center();
    harness.touch(TouchPhase::Start, below);
    harness.touch(TouchPhase::End, below);
    assert_eq!(tapped.get_untracked(), 1, "a finger still reaches the rest");
}
