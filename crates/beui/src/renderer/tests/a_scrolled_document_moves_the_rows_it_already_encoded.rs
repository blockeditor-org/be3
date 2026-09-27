use super::*;
use crate::reactive::{ForEach, Frame, build, view};
use crate::unstyled::Scroll;

const RED: Color32 = Color32::from_rgb(200, 0, 0);
const BLUE: Color32 = Color32::from_rgb(0, 0, 200);

#[test]
fn a_scrolled_document_moves_the_rows_it_already_encoded() {
    let mut document = build(|| {
        view! {
            <Scroll @test_id="scroll">
                <ForEach keys={(0..12).collect::<Vec<usize>>()}>
                    {|row: usize| view! {
                        <Frame
                            height=16.0
                            radius=0
                            color={match row % 2 {
                                0 => RED,
                                _ => BLUE,
                            }}
                        />
                    }}
                </ForEach>
            </Scroll>
        }
    });
    let scroll = document.find_test_id("scroll").expect("the scroll is built");
    let context = Context::new();
    let mut target = Target::new();
    let everything = everything();
    target.draw_in(
        &context,
        Color32::BLACK,
        |_| Repaint::Everything,
        |painter| document.show(painter.ctx(), everything),
    );
    let first = target.read();
    assert_eq!(first.pixel(10, 4), RED.to_array());
    assert_eq!(first.pixel(10, 20), BLUE.to_array());
    let encoded = target.renderer.encoded;

    document.set_scroll_offset(scroll, 16.0);
    target.draw_in(
        &context,
        Color32::BLACK,
        |output| output.repaint(Color32::BLACK),
        |painter| document.show(painter.ctx(), everything),
    );
    let scrolled = target.read();
    assert_eq!(scrolled.pixel(10, 4), BLUE.to_array());
    assert_eq!(scrolled.pixel(10, 20), RED.to_array());
    assert_eq!(scrolled.pixel(10, 60), RED.to_array());
    assert_eq!(
        target.renderer.encoded - encoded,
        1,
        "scrolling moves the rows the renderer already holds and encodes only the row it exposes"
    );
}
