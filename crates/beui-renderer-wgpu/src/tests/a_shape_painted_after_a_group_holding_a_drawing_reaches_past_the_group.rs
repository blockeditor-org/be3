use super::*;
use beui_types::Sides;
use beui_view::primitives::*;
use beui_view::reactive::*;

#[test]
fn a_shape_painted_after_a_group_holding_a_drawing_reaches_past_the_group() {
    let drawing = beui_draw::drawing(Green::default());
    let mut document = build(move || {
        view! {
            <Root clear_color={Color32::BLACK}>
                <Frame padding={Sides::all(24.0)}>
                    <Frame opacity=0.5>
                        <Drawing draw={Prop::Static(draw_gpu(Some(drawing.clone())))} />
                    </Frame>
                </Frame>
            </Root>
        }
    });
    let context = Context::new(beui_font_skrifa::SkrifaFonts::default());
    let mut target = Target::new();
    target.draw_in(
        &context,
        Color32::BLACK,
        |_| Repaint::Everything,
        |painter| {
            document.show(painter.ctx(), everything());
            painter.rect_filled(
                Rect::from_min_max(pos2(0.0, 0.0), pos2(8.0, 8.0)),
                0.0,
                Color32::from_rgb(255, 0, 0),
            );
        },
    );
    let capture = target.read();

    assert_eq!(
        capture.pixel(32, 32)[1],
        128,
        "the drawing shows at the group's opacity"
    );
    assert_eq!(
        capture.pixel(4, 4),
        [255, 0, 0, 255],
        "the shape painted after the group is not clipped to it"
    );
}
