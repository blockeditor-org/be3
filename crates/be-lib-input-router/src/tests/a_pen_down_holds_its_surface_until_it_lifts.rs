use super::*;

#[test]
fn a_pen_down_holds_its_surface_until_it_lifts() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    let pen = |phase: PenPhase, x: f32, y: f32| Input::Pen {
        phase,
        pos: pos2(x, y),
        pen: be_lib_ui_types::input::Pen {
            pressure: 0.5,
            tilt: be_lib_geometry::vec2(0.0, 0.0),
            eraser: false,
        },
    };
    let down = router.route(
        &test,
        pen(PenPhase::Down(PointerButton::Primary), 250.0, 250.0),
    );
    assert_eq!(
        targets(&down),
        vec![EDITOR, EDITOR],
        "it focuses and hears the pen"
    );
    assert_eq!(
        router.route(&test, pen(PenPhase::Move, 600.0, 600.0)),
        vec![Routed::Input {
            target: EDITOR,
            input: pen(PenPhase::Move, 400.0, 400.0),
        }]
    );
    assert_eq!(
        targets(&router.route(
            &test,
            pen(PenPhase::Up(PointerButton::Primary), 600.0, 600.0)
        )),
        vec![EDITOR]
    );
    assert_eq!(
        targets(&router.route(&test, pen(PenPhase::Move, 600.0, 600.0))),
        vec![CANVAS],
        "once lifted it hovers what is under it"
    );
}
