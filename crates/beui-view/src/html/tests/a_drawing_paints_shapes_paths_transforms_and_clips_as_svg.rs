use super::*;
use std::rc::Rc;

#[test]
fn a_drawing_paints_shapes_paths_transforms_and_clips_as_svg() {
    painting([340, 140], || {
        let draw: Draw = Rc::new(|painter: &Painter, rect: Rect| {
            painter.rect_filled(rect, 0.0, Color32::from_rgb(235, 235, 228));
            painter.rect_filled(Rect::from_min_size(pos2(10.0, 10.0), vec2(50.0, 30.0)), 0.0, BLUE);
            painter.rect_filled(
                Rect::from_min_size(pos2(70.0, 10.0), vec2(50.0, 30.0)),
                Corners::new(12.0, 0.0, 12.0, 0.0),
                GREEN,
            );
            painter.rect_stroke(
                Rect::from_min_size(pos2(130.0, 10.0), vec2(50.0, 30.0)),
                8.0,
                3.0,
                INK,
            );
            painter.line(pos2(190.0, 10.0), pos2(240.0, 40.0), 2.0, AMBER);
            painter.fill_path(
                &Path::ellipse(pos2(35.0, 80.0), vec2(25.0, 18.0)),
                FillRule::NonZero,
                Color::from(BLUE).scale_alpha(0.5),
            );
            let star = Path::polygon((0..5).map(|point| {
                let angle = std::f32::consts::PI * (point as f32 * 0.8 - 0.5);
                pos2(95.0, 82.0) + vec2(angle.cos(), angle.sin()) * 24.0
            }));
            painter.fill_path(&star, FillRule::EvenOdd, AMBER);
            let wave = Path::new().move_to(pos2(130.0, 80.0)).cubic_to(
                pos2(150.0, 40.0),
                pos2(170.0, 120.0),
                pos2(190.0, 80.0),
            );
            painter.stroke_path(
                &wave,
                &Stroke::new(5.0)
                    .cap(LineCap::Round)
                    .join(LineJoin::Round)
                    .dashed([10.0, 8.0], 0.0),
                INK,
            );
            let turned = painter.transformed(vec2(200.0, 50.0), 0.5);
            turned.rect_filled(Rect::from_min_size(pos2(0.0, 0.0), vec2(60.0, 60.0)), 10.0, GREEN);
            let clipped = painter.with_clip_rect(Rect::from_min_size(pos2(250.0, 55.0), vec2(30.0, 30.0)));
            clipped.fill_path(&Path::ellipse(pos2(280.0, 85.0), vec2(30.0, 30.0)), FillRule::NonZero, BLUE);
        });
        view! {
            <Root clear_color={PAPER}>
                <Frame align={Align2::START} padding={Sides::all(10.0)}>
                    <Drawing draw={draw} size={Some(vec2(300.0, 110.0))} />
                </Frame>
            </Root>
        }
    })
    .snapshot("html_drawing.browser");
}
