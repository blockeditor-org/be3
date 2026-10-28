use super::*;
use block_editor_beui::beui::{Rect, pos2};
use infinite_canvas_types::CanvasColor;

const COLUMNS: usize = 8;

fn grid(count: usize) -> Vec<CanvasEntity> {
    (0..count)
        .map(|index| {
            let (row, column) = (index / COLUMNS, index % COLUMNS);
            let mut shape = entity(Uuid::from_u128(index as u128 + 1));
            shape.transform = CanvasTransform::new(
                CanvasPoint::new(column as f32 * 70.0 - 245.0, row as f32 * 50.0 - 125.0),
                CanvasPoint::new(60.0, 40.0),
                0.0,
            );
            shape.style.fill = CanvasFill::Solid;
            shape.style.fill_color = CanvasColor::Rgba {
                red: 40,
                green: 90 + (column * 15) as u8,
                blue: 160,
                alpha: 255,
            };
            if index % 5 == 0 {
                shape.shape = CanvasShape::Text;
                shape.text = format!("T{index}").as_str().into();
                shape.placeholder = "Text".to_owned();
            }
            shape
        })
        .collect()
}

fn work(editor: &mut BeuiTest<CanvasApp>, view: Rect, scale: f32) -> (usize, usize) {
    editor.set_view(view, scale);
    editor.step(Vec::new());
    let work = editor.document().performance().latest.work;
    (work.painted_nodes, work.placed)
}

fn moved(count: usize) -> (BeuiTest<CanvasApp>, [(usize, usize); 2]) {
    let mut editor = editor(&grid(count));
    let size = editor.rect().size();
    let view = Rect::from_min_size(pos2(0.0, 0.0), size);
    editor.set_view(view, 1.0);
    editor.run();
    let panned = work(&mut editor, view.translate(Vec2::new(40.0, 25.0)), 1.0);
    let zoomed = work(
        &mut editor,
        Rect::from_center_size(view.center(), size * 1.5),
        1.5,
    );
    (editor, [panned, zoomed])
}

#[test]
fn panning_and_zooming_paints_no_entity_again() {
    let (_, few) = moved(6);
    let (mut editor, many) = moved(48);
    assert_eq!(
        few, many,
        "a pan and a zoom painted and laid out more nodes for 48 entities than for 6"
    );
    editor.advance(std::time::Duration::from_millis(500));
    editor.run();
    editor.snapshot("a_zoomed_canvas_scales_its_shapes");
}
