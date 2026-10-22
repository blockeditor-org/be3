use super::*;
use beui_core::node::SpaceId;
use beui_core::painter::Zoom;

fn inked_width(capture: &Capture) -> u32 {
    let columns: Vec<u32> = (0..SIZE)
        .filter(|x| (0..SIZE).any(|y| capture.pixel(*x, y)[0] > 64))
        .collect();
    match (columns.first(), columns.last()) {
        (Some(first), Some(last)) => last - first + 1,
        _ => 0,
    }
}

fn written(zoom: Zoom) -> Capture {
    let host = Document::new().create_canvas().id();
    capture(Color32::BLACK, |painter| {
        let zoomed = painter.zoomed(SpaceId::inside(host, 1), vec2(2.0, 2.0), zoom, everything());
        zoomed.text(
            pos2(0.0, 0.0),
            "HH",
            FontId::proportional(12.0),
            Color32::WHITE,
        );
    })
}

#[test]
fn zoomed_text_stretches_the_glyphs_it_was_rasterised_at() {
    let plain = inked_width(&written(Zoom::NONE));
    let stretched = inked_width(&written(Zoom {
        scale: 2.0,
        raster: 1.0,
    }));
    assert!(plain > 4, "the text inked {plain} columns");
    assert!(
        stretched.abs_diff(plain * 2) <= 2,
        "text zoomed to twice its size before it is rasterised again inks {stretched} columns, \
         not about twice the {plain} it inks unzoomed"
    );
}
