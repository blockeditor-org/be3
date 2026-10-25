use super::*;
use block_editor_beui::be_block::be_model::RichText;
use block_editor_beui::beui::{Key, Modifiers};
use infinite_canvas_types::{CanvasStrokeStyle, CanvasTextSize};
use text_editor_core::schema::basic::{BOLD, ITALIC, STRIKETHROUGH, UNDERLINE, mark};

const EXPECTED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="288" height="136" viewBox="-52 -40 288 136">
<rect x="-52" y="-40" width="288" height="136" fill="#0e1117"/>
<path d="M-30 -20 L30 -20 L30 20 L-30 20 Z" fill="#fac850"/>
<path d="M-30 -20 L30 -20 L30 20 L-30 20 Z" fill="none" stroke="#1e1e1e" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
<text font-family="sans-serif" font-size="16" fill="#1e1e1e"><tspan x="-20.4" y="6">A &amp; </tspan><tspan x="9.93" y="6" text-decoration="line-through">B</tspan></text>
<path d="M110 0 C110 11.05 96.57 20 80 20 C63.43 20 50 11.05 50 0 C50 -11.05 63.43 -20 80 -20 C96.57 -20 110 -11.05 110 0 Z" fill="none" stroke="#1e1e1e" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
<path d="M160 -20 L190 0 L160 20 L130 0 Z" fill="none" stroke="#1e1e1e" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" stroke-dasharray="6 6"/>
<path d="M-30 60 L30 60" fill="none" stroke="#e6ebf3" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
<path d="M30 60 L22 56.4 M30 60 L22 63.6" fill="none" stroke="#e6ebf3" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
<path d="M60 60 Q60 59.99 60.5 60.01 Q61 60.02 61.97 60.12 Q62.94 60.22 63.88 60.41 Q64.82 60.6 65.74 60.88 Q66.65 61.16 70.87 62.57 Q75.08 63.97 78.83 65.22 Q82.58 66.47 83.44 66.76 Q84.29 67.04 85.15 67.34 Q86.01 67.63 86.84 67.99 Q87.68 68.34 88.47 68.84 Q89.26 69.34 89.63 69.67 Q90 69.99 90 70 Q90 70 90 70 Q90 70.01 89.51 70.05 Q89.02 70.08 88.08 70.01 Q87.15 69.93 86.27 69.72 Q85.38 69.5 84.52 69.22 Q83.66 68.94 82.8 68.66 Q81.95 68.37 78.2 67.12 Q74.45 65.87 70.23 64.47 Q66.02 63.06 65.12 62.74 Q64.22 62.41 63.35 62 Q62.48 61.59 61.65 61.09 Q60.81 60.58 60.4 60.29 Q60 60.01 60 60 Z" fill="#e6ebf3"/>
<text font-family="sans-serif" font-size="20" fill="#1e1e1e"><tspan x="104" y="65.3" font-weight="bold">Hi</tspan><tspan x="129.33" y="65.3" font-style="italic" text-decoration="underline">there</tspan></text>
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
    editor.key(
        Key::P,
        Modifiers {
            shift: true,
            ..ctrl
        },
    );
    editor.run();
    editor.type_text(typed);
    editor.run();
    editor.key(Key::Enter, Modifiers::NONE);
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
    rectangle.text = RichText::new("A & B", vec![mark(4..5, STRIKETHROUGH)]);
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
    text.transform.size = CanvasPoint::new(120.0, 40.0);
    text.text = RichText::new(
        "Hi there",
        vec![mark(0..2, BOLD), mark(3..8, ITALIC), mark(3..8, UNDERLINE)],
    );
    let mut editor = editor(&[rectangle, ellipse, diamond, arrow, freehand, text]);

    editor.key(
        Key::A,
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    );
    editor.run();
    palette_runs(&mut editor, "copy as svg");

    let copied = editor.take_copied_text();
    let [svg] = copied.as_slice() else {
        panic!("Copy as SVG puts one SVG on the clipboard, put {copied:?}");
    };
    assert_eq!(svg, EXPECTED, "the SVG changed:\n{svg}");
}
