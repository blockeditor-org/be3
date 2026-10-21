use super::*;
use block_editor_beui::be_block::canvas::{CanvasStrokeStyle, CanvasTextSize};
use block_editor_beui::beui::{Key, Modifiers};

const EXPECTED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="262" height="136" viewBox="-52 -40 262 136">
<rect x="-52" y="-40" width="262" height="136" fill="#0e1117"/>
<path d="M-30 -20 L30 -20 L30 20 L-30 20 Z" fill="#fac850"/>
<path d="M-30 -20 L30 -20 L30 20 L-30 20 Z" fill="none" stroke="#1e1e1e" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
<text font-family="sans-serif" font-size="16" fill="#1e1e1e"><tspan x="-19" y="5">A &amp; B</tspan></text>
<path d="M110 0 C110 11.05 96.57 20 80 20 C63.43 20 50 11.05 50 0 C50 -11.05 63.43 -20 80 -20 C96.57 -20 110 -11.05 110 0 Z" fill="none" stroke="#1e1e1e" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
<path d="M160 -20 L190 0 L160 20 L130 0 Z" fill="none" stroke="#1e1e1e" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" stroke-dasharray="6 6"/>
<path d="M-30 60 L30 60" fill="none" stroke="#e6ebf3" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
<path d="M30 60 L22 56.4 M30 60 L22 63.6" fill="none" stroke="#e6ebf3" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
<path d="M60 59.99 L64.61 60.54 L70.77 62.54 L77.7 64.85 L90 69.99 L90 70 L90 70.01 L77.07 66.74 L70.14 64.43 L64.01 62.33 L60 60.01 Z" fill="#e6ebf3"/>
<text font-family="sans-serif" font-size="20" fill="#1e1e1e"><tspan x="134" y="66">Hi</tspan></text>
</svg>
"##;

fn placed(id: u128, shape: CanvasShape, center: CanvasPoint) -> CanvasEntity {
    let mut entity = shaped(Uuid::from_u128(id), shape);
    entity.transform = CanvasTransform::new(center, CanvasPoint::new(60.0, 40.0), 0.0);
    entity.style.stroke_color = CanvasColor::Rgba {
        red: 30,
        green: 30,
        blue: 30,
        alpha: 255,
    };
    entity
}

pub(super) fn palette_runs(editor: &mut BeuiTest<CanvasApp>, typed: &str) {
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    editor.key_press_modifiers(
        Modifiers {
            shift: true,
            ..ctrl
        },
        Key::P,
    );
    editor.run();
    editor.text(typed);
    editor.run();
    editor.key_press(Key::Enter);
    editor.run();
}

#[test]
fn copying_as_svg_writes_every_shape_kind() {
    let mut rectangle = placed(1, CanvasShape::Rectangle, CanvasPoint::new(0.0, 0.0));
    rectangle.style.fill = CanvasFill::Solid;
    rectangle.style.fill_color = CanvasColor::Rgba {
        red: 250,
        green: 200,
        blue: 80,
        alpha: 255,
    };
    rectangle.text = "A & B".to_owned();
    rectangle.style.text_size = CanvasTextSize::Small;
    let ellipse = placed(2, CanvasShape::Ellipse, CanvasPoint::new(80.0, 0.0));
    let mut diamond = placed(3, CanvasShape::Diamond, CanvasPoint::new(160.0, 0.0));
    diamond.style.stroke_style = CanvasStrokeStyle::Dashed;
    let mut arrow = path(
        Uuid::from_u128(4),
        &[CanvasPoint::new(-30.0, 60.0), CanvasPoint::new(30.0, 60.0)],
    );
    arrow.style.end_arrowhead = CanvasArrowhead::Arrow;
    let mut freehand = path(
        Uuid::from_u128(5),
        &[CanvasPoint::new(60.0, 60.0), CanvasPoint::new(90.0, 70.0)],
    );
    freehand.shape = CanvasShape::Freehand;
    freehand.pressures = vec![0.5, 0.5];
    let mut text = placed(6, CanvasShape::Text, CanvasPoint::new(160.0, 60.0));
    text.text = "Hi".to_owned();
    let mut editor = editor(&[rectangle, ellipse, diamond, arrow, freehand, text]);

    editor.key_press_modifiers(
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
        Key::A,
    );
    editor.run();
    palette_runs(&mut editor, "copy as svg");

    let copied = editor.take_copied_text();
    let [svg] = copied.as_slice() else {
        panic!("Copy as SVG puts one SVG on the clipboard, put {copied:?}");
    };
    assert_eq!(svg, EXPECTED, "the SVG changed:\n{svg}");
}
