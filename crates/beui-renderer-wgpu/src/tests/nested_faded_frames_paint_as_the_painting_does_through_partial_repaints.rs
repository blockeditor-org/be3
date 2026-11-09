use super::*;
use beui_types::Sides;
use beui_view::primitives::*;
use beui_view::reactive::*;
use std::cell::Cell;
use std::rc::Rc;

/// A faded frame inside a faded frame, drawn on the GPU and rasterized from the same
/// frame's painting: first whole, then after the inner opacity changes (a repaint
/// whose scissor cuts through the outer group), then after the outer one changes.
#[test]
fn nested_faded_frames_paint_as_the_painting_does_through_partial_repaints() {
    let held = Rc::new(Cell::new(None));
    let mut document = build({
        let held = held.clone();
        move || {
            let outer = create_signal(0.5_f32);
            let inner = create_signal(0.5_f32);
            held.set(Some((outer, inner)));
            view! {
                <Root clear_color={Color32::BLACK}>
                    <Frame opacity={outer}>
                        <Frame fill=Color32::WHITE padding={Sides::new(16.0, 16.0, 16.0, 16.0)}>
                            <Frame opacity={inner}>
                                <Frame
                                    fill={Color32::from_rgb(255, 0, 0)}
                                    padding={Sides::new(8.0, 8.0, 0.0, 0.0)}
                                >
                                    <Frame fill={Color32::from_rgb(0, 0, 255)} />
                                </Frame>
                            </Frame>
                        </Frame>
                    </Frame>
                </Root>
            }
        }
    });
    let (outer, inner) = held.get().expect("the view is built");
    let context = Context::new(beui_font_freetype::FreetypeFonts::default());
    let mut target = Target::in_format(wgpu::TextureFormat::Rgba8Unorm);
    let mut draw = |document: &mut Document, whole: bool| {
        let output = target.draw_in(
            &context,
            Color32::BLACK,
            |output| match whole {
                true => Repaint::Everything,
                false => output.repaint(Color32::BLACK),
            },
            |painter| document.show(painter.ctx(), everything()),
        );
        let painting = beui_core::painting::painting(
            output.main(),
            vec2(SIZE as f32, SIZE as f32),
            1.0,
            Color32::BLACK,
            beui_core::painting::Punches::Shown,
        )
        .expect("the frame paints");
        let expected = beui_core::painting::rasterize(&painting.frame, [0, 0, 0, 255], |key| {
            painting.textures.get(&key)
        })
        .expect("the painting rasterizes");
        let drawn = target.read();
        for y in 0..SIZE {
            for x in 0..SIZE {
                let want = expected.rgba[(y * SIZE + x) as usize];
                let got = drawn.pixel(x, y);
                assert!(
                    want.iter()
                        .zip(got)
                        .all(|(want, got)| want.abs_diff(got) <= 3),
                    "at ({x}, {y}) the GPU drew {got:?} where the painting has {want:?}"
                );
            }
        }
        drawn
    };

    let drawn = draw(&mut document, true);
    let [red, green, blue, _] = drawn.pixel(40, 40);
    assert!(
        red.abs_diff(green) <= 2 && blue > red,
        "the blue square hides the red one under it: {:?}",
        [red, green, blue]
    );

    inner.set(0.25);
    draw(&mut document, false);

    outer.set(0.75);
    draw(&mut document, false);
}
