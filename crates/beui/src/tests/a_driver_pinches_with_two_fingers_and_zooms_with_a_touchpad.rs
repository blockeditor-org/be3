use super::*;
use crate::input::ZoomGesture;
use crate::reactive::{Frame, Interactive, build, view};

#[test]
fn a_driver_pinches_with_two_fingers_and_zooms_with_a_touchpad() {
    let zoom = Rc::new(Cell::new(1.0_f32));
    let document = build({
        let zoom = zoom.clone();
        move || {
            view! {
                <Interactive
                    @test_id={"surface"}
                    on_zoom={move |gesture: ZoomGesture| zoom.set(zoom.get() * gesture.factor)}
                >
                    <Frame width=300.0 height=300.0 />
                </Interactive>
            }
        }
    });
    let mut driven = Driven::new(document);

    driven
        .ask(&["pinch", "#surface", "2"])
        .expect("two fingers spread");
    assert!(
        (zoom.get() - 2.0).abs() < 0.01,
        "spreading doubled the zoom to {}",
        zoom.get()
    );
    driven
        .ask(&["zoom", "#surface", "0.5"])
        .expect("the touchpad pinches");
    assert!(
        (zoom.get() - 1.0).abs() < 0.01,
        "the touchpad halved it to {}",
        zoom.get()
    );

    driven
        .ask(&["touch", "down", "0=100,150", "1=200,150"])
        .expect("two fingers land in one frame");
    let state = driven.ask(&["state"]).expect("the state is read");
    assert!(
        state.contains("finger 0 at 100,150, finger 1 at 200,150"),
        "{state}"
    );
    driven
        .ask(&["touch", "move", "0=50,150", "1=250,150"])
        .expect("both fingers move in one frame");
    driven.ask(&["touch", "up"]).expect("every finger lifts");
    assert!(
        (zoom.get() - 2.0).abs() < 0.01,
        "the fingers doubled it again to {}",
        zoom.get()
    );
}
