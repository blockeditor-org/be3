use super::*;
use std::rc::Rc;

fn filled(color: Color32) -> Draw {
    Rc::new(move |painter: &Painter, rect: Rect| painter.rect_filled(rect, 0.0, color))
}

#[test]
fn a_drawing_redraws_its_svg_when_its_callback_changes() {
    let scope = ::reactive::Scope::detached();
    let (color, sized, roomless) = scope.run(|| {
        let color = create_signal(BLUE);
        let draw = Prop::dynamic(move || filled(color.get()));
        let sized = view! {
            <Drawing draw={draw} size={Some(vec2(40.0, 20.0))} />
        };
        let roomless = view! {
            <Drawing draw={Prop::dynamic(move || filled(color.get()))} />
        };
        (color, sized, roomless)
    });
    ::reactive::flush();
    let markup = |node: NodeId| beui_core_html::document::element(node).unwrap().to_html();

    let before = markup(sized);
    assert!(before.contains("viewBox=\"0 0 40 20\""), "{before}");
    assert!(before.contains("fill: #466edc;"), "{before}");
    assert!(
        !markup(roomless).contains("<svg"),
        "a drawing with no size waits for its measurement"
    );

    color.set(GREEN);
    ::reactive::flush();
    let after = markup(sized);
    assert!(after.contains("fill: #3caa6e;"), "{after}");
    assert!(!after.contains("#466edc"), "{after}");
    scope.dispose();
}
