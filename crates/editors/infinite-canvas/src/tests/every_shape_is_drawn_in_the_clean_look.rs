use super::*;
use block_editor_beui::be_block::canvas::{CanvasStrokeStyle, CanvasTextSize};

fn colored(red: u8, green: u8, blue: u8) -> CanvasColor {
    CanvasColor::Rgba {
        red,
        green,
        blue,
        alpha: 255,
    }
}

fn placed(id: u128, shape: CanvasShape, center: CanvasPoint) -> CanvasEntity {
    let mut entity = shaped(Uuid::from_u128(id), shape);
    entity.transform = CanvasTransform::new(center, CanvasPoint::new(120.0, 80.0), 0.0);
    entity
}

#[test]
fn every_shape_is_drawn_in_the_clean_look() {
    let mut ellipse = placed(1, CanvasShape::Ellipse, CanvasPoint::new(-160.0, -100.0));
    ellipse.style.fill = CanvasFill::Solid;
    ellipse.style.fill_color = colored(250, 200, 80);
    let mut diamond = placed(2, CanvasShape::Diamond, CanvasPoint::new(0.0, -100.0));
    diamond.style.stroke_style = CanvasStrokeStyle::Dashed;
    diamond.style.stroke_color = colored(25, 113, 194);
    let mut labelled = placed(3, CanvasShape::Rectangle, CanvasPoint::new(160.0, -100.0));
    labelled.text = "Label".to_owned();
    labelled.style.text_size = CanvasTextSize::Small;

    let mut polyline = path(
        Uuid::from_u128(4),
        &[
            CanvasPoint::new(-220.0, 20.0),
            CanvasPoint::new(-140.0, 80.0),
            CanvasPoint::new(-60.0, 20.0),
        ],
    );
    polyline.style.start_arrowhead = CanvasArrowhead::Arrow;
    polyline.style.end_arrowhead = CanvasArrowhead::Arrow;
    polyline.style.stroke_style = CanvasStrokeStyle::Dotted;

    let mut curve = path(
        Uuid::from_u128(5),
        &[CanvasPoint::new(0.0, 20.0), CanvasPoint::new(0.0, 100.0)],
    );
    let mut world = curve.world_points();
    world[0].handle_out = Some(CanvasPoint::new(80.0, 20.0));
    world[1].handle_in = Some(CanvasPoint::new(-80.0, 100.0));
    curve.set_world_points(world);
    curve.text = "curve".to_owned();
    curve.style.text_size = CanvasTextSize::Small;

    let mut freehand = path(
        Uuid::from_u128(6),
        &[
            CanvasPoint::new(120.0, 30.0),
            CanvasPoint::new(160.0, 70.0),
            CanvasPoint::new(200.0, 40.0),
        ],
    );
    freehand.shape = CanvasShape::Freehand;
    freehand.pressures = vec![0.2, 0.9, 0.5];

    let mut editor = editor(&[ellipse, diamond, labelled, polyline, curve, freehand]);
    editor.run();

    editor.snapshot("every_shape_is_drawn_in_the_clean_look");
}
