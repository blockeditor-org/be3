use super::*;
use block_editor_beui::be_block::be_model::RichText;
use block_editor_beui::be_block::canvas::{CanvasStrokeStyle, CanvasTextSize};
use block_editor_beui::beui::{Key, Modifiers};
use text_editor_core::schema::basic::{BOLD, ITALIC, STRIKETHROUGH, UNDERLINE, mark};

const EXPECTED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="288" height="136" viewBox="-52 -40 288 136">
<rect x="-52" y="-40" width="288" height="136" fill="#0e1117"/>
<path d="M-30 -20 L30 -20 L30 20 L-30 20 Z" fill="#fac850"/>
<path d="M-30 -20 L30 -20 L30 20 L-30 20 Z" fill="none" stroke="#1e1e1e" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
<text font-family="sans-serif" font-size="16" fill="#1e1e1e"><tspan x="-19.5" y="5">A &amp; </tspan><tspan x="8.5" y="5" text-decoration="line-through">B</tspan></text>
<path d="M110 0 C110 11.05 96.57 20 80 20 C63.43 20 50 11.05 50 0 C50 -11.05 63.43 -20 80 -20 C96.57 -20 110 -11.05 110 0 Z" fill="none" stroke="#1e1e1e" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
<path d="M160 -20 L190 0 L160 20 L130 0 Z" fill="none" stroke="#1e1e1e" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" stroke-dasharray="6 6"/>
<path d="M-30 60 L30 60" fill="none" stroke="#e6ebf3" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
<path d="M30 60 L22 56.4 M30 60 L22 63.6" fill="none" stroke="#e6ebf3" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
<path d="M60 60 Q60 59.99 60.47 60.01 Q60.94 60.02 61.84 60.1 Q62.75 60.19 63.68 60.37 Q64.61 60.54 65.45 60.79 Q66.29 61.04 67.18 61.34 Q68.06 61.63 68.96 61.93 Q69.87 62.24 73.78 63.54 Q77.7 64.85 78.5 65.11 Q79.3 65.38 80.17 65.67 Q81.03 65.96 81.94 66.26 Q82.84 66.56 83.78 66.87 Q84.71 67.18 85.65 67.52 Q86.59 67.85 87.49 68.29 Q88.38 68.73 88.8 69.02 Q89.22 69.31 89.61 69.65 Q90 69.99 90 70 Q90 70 90 70 Q90 70.01 89.48 70.05 Q88.96 70.08 88.45 70.06 Q87.95 70.04 86.96 69.86 Q85.98 69.67 85.03 69.38 Q84.08 69.08 83.14 68.77 Q82.21 68.46 81.3 68.16 Q80.4 67.85 79.53 67.57 Q78.67 67.28 77.87 67.01 Q77.07 66.74 73.15 65.44 Q69.23 64.13 68.33 63.83 Q67.43 63.53 66.54 63.24 Q65.66 62.94 64.84 62.64 Q64.01 62.33 63.16 61.92 Q62.31 61.5 61.54 61.02 Q60.76 60.55 60.38 60.28 Q60 60.01 60 60 Z" fill="#e6ebf3"/>
<text font-family="sans-serif" font-size="20" fill="#1e1e1e"><tspan x="104" y="65.3" font-weight="bold">Hi</tspan><tspan x="128" y="65.3" font-style="italic" text-decoration="underline">there</tspan></text>
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
