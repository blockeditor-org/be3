use super::*;
use beui_components_unstyled::Scroll;
use beui_view::reactive::{ForEach, Frame, List, build, view};

const SHADES: [Color32; 3] = [
    Color32::from_rgb(200, 40, 0),
    Color32::from_rgb(0, 160, 60),
    Color32::from_rgb(30, 60, 220),
];

fn scrolling() -> Document {
    build(|| {
        view! {
            <Frame color=Color32::from_rgb(20, 20, 20) radius=0>
                <List spacing=0.0>
                    <Frame height=12.0 color=Color32::WHITE radius=0 />
                    <Frame
                        height=40.0
                        color=Color32::from_rgb(20, 20, 20)
                        radius=0
                        outline=Color32::from_rgb(250, 250, 120)
                        outline_width=1.0
                        outline_visible=true
                    >
                        <Scroll @test_id="scroll">
                            <ForEach keys={(0..20).collect::<Vec<usize>>()}>
                                {|row: usize| view! {
                                    <Frame height=10.0>
                                        <Frame
                                            width=30.0
                                            height=6.0
                                            radius=0
                                            color={SHADES[row % SHADES.len()]}
                                        />
                                    </Frame>
                                }}
                            </ForEach>
                        </Scroll>
                    </Frame>
                </List>
            </Frame>
        }
    })
}

#[test]
fn copying_a_scrolled_region_paints_what_a_full_repaint_would() {
    beui_core::document::verify_paint(true);
    let mut document = scrolling();
    let scroll = document
        .find_test_id("scroll")
        .expect("the scroll is built");
    let context = Context::new(beui_font_freetype::FreetypeFonts::default());
    let mut target = Target::new();
    let everything = everything();
    target.draw_moving(&context, Color32::BLACK, |painter| {
        document.show(painter.ctx(), everything)
    });
    let mut moves = 0;
    for offset in [7.0, 20.0, 13.0] {
        document.set_scroll_offset(scroll, offset);
        let moved = target.draw_moving(&context, Color32::BLACK, |painter| {
            document.show(painter.ctx(), everything)
        });
        moves += usize::from(moved.is_some());

        let mut fresh = scrolling();
        let fresh_scroll = fresh.find_test_id("scroll").expect("the scroll is built");
        let mut whole = Target::new();
        let fresh_context = Context::new(beui_font_freetype::FreetypeFonts::default());
        whole.draw_moving(&fresh_context, Color32::BLACK, |painter| {
            fresh.show(painter.ctx(), everything)
        });
        fresh.set_scroll_offset(fresh_scroll, offset);
        whole.draw_in(
            &fresh_context,
            Color32::BLACK,
            |_| Repaint::Everything,
            |painter| fresh.show(painter.ctx(), everything),
        );
        let (copied, painted) = (target.read(), whole.read());
        for y in 0..SIZE {
            for x in 0..SIZE {
                assert_eq!(
                    copied.pixel(x, y),
                    painted.pixel(x, y),
                    "scrolled to {offset}, the copied frame differs from a full repaint at {x},{y}"
                );
            }
        }
    }
    assert_eq!(moves, 3, "each scroll copies what it already showed");
}
