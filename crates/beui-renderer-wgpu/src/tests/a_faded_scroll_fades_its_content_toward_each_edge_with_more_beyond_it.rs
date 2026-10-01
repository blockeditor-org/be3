use super::*;
use beui_components_unstyled::{Scroll, ScrollbarStyle};
use beui_view::reactive::{ForEach, Frame, build, view};

#[test]
fn a_faded_scroll_fades_its_content_toward_each_edge_with_more_beyond_it() {
    let mut document = build(|| {
        view! {
            <Scroll @test_id="scroll" scrollbar={ScrollbarStyle::default().fading(16.0)}>
                <ForEach keys={(0..8).collect::<Vec<usize>>()}>
                    {|_: usize| view! {
                        <Frame height=16.0 radius=0 color=Color32::WHITE />
                    }}
                </ForEach>
            </Scroll>
        }
    });
    let scroll = document
        .find_test_id("scroll")
        .expect("the scroll is built");
    let context = Context::new(beui_font_freetype::FreetypeFonts::default());
    let everything = everything();
    let shown = |document: &mut Document| {
        let mut target = Target::new();
        target.draw_in(
            &context,
            Color32::BLACK,
            |_| Repaint::Everything,
            |painter| document.show(painter.ctx(), everything),
        );
        let capture = target.read();
        [1, 32, 62].map(|y| capture.pixel(10, y)[0])
    };

    let [top, middle, bottom] = shown(&mut document);
    assert_eq!(top, 255, "nothing lies above the top, so it does not fade");
    assert_eq!(middle, 255);
    assert!(
        bottom < 128,
        "the rows below fade out at the bottom: {bottom}"
    );

    document.set_scroll_offset(scroll, 32.0);
    let [top, middle, bottom] = shown(&mut document);
    assert!(top < 128, "rows scrolled past fade out at the top: {top}");
    assert_eq!(middle, 255);
    assert!(bottom < 128, "{bottom}");

    document.set_scroll_offset(scroll, 64.0);
    let [top, middle, bottom] = shown(&mut document);
    assert!(top < 128, "{top}");
    assert_eq!(middle, 255);
    assert_eq!(bottom, 255, "scrolled to the end, the bottom does not fade");
}
