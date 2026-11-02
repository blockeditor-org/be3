use super::*;
use beui_view::primitives::*;
use beui_view::reactive::*;

fn rows(fill: Color32) -> Document {
    build(move || {
        view! {
            <Root clear_color={beui_types::color::Color32::BLACK}>
                <Frame height=64.0 radius=0.0 fill={fill} />
            </Root>
        }
    })
}

#[test]
fn a_renderers_encoded_rows_survive_another_renderer_painting_between_its_frames() {
    let mut red = rows(Color32::from_rgb(200, 0, 0));
    let mut blue = rows(Color32::from_rgb(0, 0, 200));
    let red_context = Context::new(beui_font_freetype::FreetypeFonts::default());
    let blue_context = Context::new(beui_font_freetype::FreetypeFonts::default());
    let mut first = Target::new();
    let mut second = Target::sharing(&first);
    let everything = everything();

    first.draw_in(
        &red_context,
        Color32::BLACK,
        |_| Repaint::Everything,
        |painter| red.show(painter.ctx(), everything),
    );
    let encoded = first.renderer.encoded;
    second.draw_in(
        &blue_context,
        Color32::BLACK,
        |_| Repaint::Everything,
        |painter| blue.show(painter.ctx(), everything),
    );
    first.draw_in(
        &red_context,
        Color32::BLACK,
        |_| Repaint::Everything,
        |painter| red.show(painter.ctx(), everything),
    );

    assert_eq!(
        first.renderer.encoded, encoded,
        "the first renderer reuses every row it encoded although the second painted in between"
    );
    assert_eq!(first.read().pixel(10, 4), [200, 0, 0, 255]);
    assert_eq!(second.read().pixel(10, 4), [0, 0, 200, 255]);
}
