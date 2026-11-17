use super::*;
use beui_types::Sides;
use beui_view::primitives::*;
use beui_view::reactive::*;

#[test]
fn a_screen_in_a_composited_window_blends_over_the_ui_beneath_it_and_fades_with_its_group() {
    beui_draw::compose::composite_windows();
    let mut target = Target::new();
    target.renderer.set_window(true);
    let screen = |id| be_lib_layers::Surface::Screen {
        instance: id,
        surface: 0,
    };
    // Half-covered blue, premultiplied, as an app's translucent pixels are.
    let translucent = texture_of(&target, 1, &[[0, 0, 128, 128]]);
    let opaque = texture_of(&target, 1, &[[0, 0, 255, 255]]);
    let translucent = beui_draw::drawing(Shows::new(translucent, screen(1), 1));
    let faded = beui_draw::drawing(Shows::new(opaque, screen(2), 2));
    let context = Context::new(beui_font_freetype::FreetypeFonts::default());
    let mut show = |drawing: beui_core::drawing::Drawing, opacity: f32| {
        let mut document = build(move || {
            view! {
                <Root clear_color={Color32::BLACK}>
                    <Frame fill={Color32::from_rgb(255, 0, 0)} padding={Sides::all(8.0)}>
                        <Frame opacity={opacity}>
                            <Drawing draw={Prop::Static(draw_gpu(Some(drawing.clone())))} />
                        </Frame>
                    </Frame>
                </Root>
            }
        });
        target.draw_in(
            &context,
            Color32::BLACK,
            |_| Repaint::Everything,
            |painter| document.show(painter.ctx(), everything()),
        );
        target.read().pixel(32, 32)
    };
    let [red, _, blue, _] = show(translucent, 1.0);
    assert!(
        red > 100 && blue > 100,
        "a translucent screen blends over the UI painted beneath it: {:?}",
        [red, blue]
    );
    let [red, _, blue, _] = show(faded, 0.5);
    assert!(
        red > 100 && blue > 100,
        "a screen in a faded group shows at the group's opacity over the UI beneath it: {:?}",
        [red, blue]
    );
    let layers = target.renderer.layers().expect("the window is composited");
    assert_eq!(
        layers.layers.len(),
        2,
        "the screen is listed, with the UI's surface: {layers:?}"
    );
}
