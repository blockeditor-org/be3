use super::*;
use beui_types::Sides;
use beui_view::primitives::*;
use beui_view::reactive::*;

#[test]
fn overlapping_shapes_in_a_faded_frame_do_not_show_through_each_other() {
    let mut document = build(|| {
        view! {
            <Root clear_color={Color32::BLACK}>
                <Frame opacity=0.5>
                    <Frame fill=Color32::WHITE padding={Sides::new(16.0, 16.0, 0.0, 0.0)}>
                        <Frame fill={Color32::from_rgb(255, 0, 0)} />
                    </Frame>
                </Frame>
            </Root>
        }
    });
    let context = Context::new(beui_font_freetype::FreetypeFonts::default());
    let mut target = Target::new();
    target.draw_in(
        &context,
        Color32::BLACK,
        |_| Repaint::Everything,
        |painter| document.show(painter.ctx(), everything()),
    );
    let capture = target.read();

    let [red, green, blue, _] = capture.pixel(40, 40);
    assert!(
        red > 150 && green < 4 && blue < 4,
        "the red square shows at half strength with none of the white under it: {:?}",
        [red, green, blue]
    );
    let [red, green, blue, _] = capture.pixel(4, 4);
    assert!(
        red > 150 && red == green && green == blue,
        "the white square shows grey where nothing covers it: {:?}",
        [red, green, blue]
    );
}
