use super::*;
use std::rc::Rc;

#[test]
fn text_in_a_drawing_lands_where_a_text_node_puts_it() {
    painting([460, 140], || {
        let font = FontId::proportional(16.0);
        let fill = Color32::from_rgb(225, 228, 236);
        let draw: Draw = Rc::new(move |painter: &Painter, rect: Rect| {
            let label = painter.lay_out_text("Placed at its corner", font, TextLayout::DEFAULT);
            painter.rect_filled(Rect::from_min_size(rect.min, label.size()), 0.0, fill);
            painter.draw_text(rect.min, &label, INK);
            let wrapped = painter.lay_out_text(
                "Measured, wrapped and centred as it is drawn",
                font,
                TextLayout {
                    align: TextAlign::Center,
                    ..TextLayout::wrapped(140.0)
                },
            );
            let at = rect.min + vec2(0.0, label.size().y + 12.0);
            painter.rect_filled(Rect::from_min_size(at, wrapped.size()), 0.0, fill);
            for line in wrapped.lines() {
                painter.line(
                    at + vec2(line.rect.left(), line.baseline),
                    at + vec2(line.rect.right(), line.baseline),
                    1.0,
                    BLUE,
                );
            }
            painter.draw_text(at, &wrapped, INK);
        });
        view! {
            <Root clear_color={PAPER}>
                <Frame align={Align2::START} padding={Sides::all(10.0)}>
                    <List direction={Direction::Horizontal} align={Align::Start} spacing={20.0}>
                        <Frame fill={fill}>
                            <Text color={INK} font={font}>"Placed at its corner"</Text>
                        </Frame>
                        <Drawing draw={draw} size={Some(vec2(200.0, 110.0))} />
                    </List>
                </Frame>
            </Root>
        }
    })
    .snapshot("html_text_in_a_drawing.browser");
}
