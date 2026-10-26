use block_editor_beui::beui::{Vec2, vec2};
use infinite_canvas_types::Canvas;
use infinite_canvas_types::CanvasContent;
use infinite_canvas_types::{
    CanvasEntity, CanvasPoint, CanvasShape, CanvasTextAlign, CanvasTextSize, CanvasTextStyle,
    CanvasTextVerticalAlign, CanvasTextWeight, CanvasTransform,
};

const DEFAULT_SLIDE_SIZE: Vec2 = vec2(960.0, 540.0);

const TITLE_FONT_SIZE: f32 = 54.0;
const SUBTITLE_FONT_SIZE: f32 = 26.0;
const HEADER_FONT_SIZE: f32 = 40.0;
const BODY_FONT_SIZE: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlideTemplate {
    Title,
    Regular,
    Blank,
}

impl SlideTemplate {
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "title-slide" => Some(Self::Title),
            "regular-slide" => Some(Self::Regular),
            "blank-slide" => Some(Self::Blank),
            _ => None,
        }
    }
}

fn template_text_entity(
    center: CanvasPoint,
    size: CanvasPoint,
    placeholder: &str,
    font_size: f32,
    text_style: CanvasTextStyle,
) -> CanvasEntity {
    let mut entity = CanvasEntity::new(CanvasShape::Text, CanvasTransform::new(center, size, 0.0));
    entity.placeholder = placeholder.into();
    entity.text_style = text_style;
    entity.style.text_size = CanvasTextSize::Custom(font_size);
    entity
}

fn template_entities(template: SlideTemplate) -> Vec<CanvasEntity> {
    match template {
        SlideTemplate::Blank => Vec::new(),
        SlideTemplate::Title => vec![
            template_text_entity(
                CanvasPoint::new(0.0, -40.0),
                CanvasPoint::new(820.0, 110.0),
                "Title",
                TITLE_FONT_SIZE,
                CanvasTextStyle {
                    weight: CanvasTextWeight::Bold,
                    alignment: CanvasTextAlign::Center,
                    vertical_alignment: CanvasTextVerticalAlign::Middle,
                    line_height: 1.2,
                    wrap: false,
                },
            ),
            template_text_entity(
                CanvasPoint::new(0.0, 70.0),
                CanvasPoint::new(700.0, 60.0),
                "Subtitle",
                SUBTITLE_FONT_SIZE,
                CanvasTextStyle {
                    weight: CanvasTextWeight::Regular,
                    alignment: CanvasTextAlign::Center,
                    vertical_alignment: CanvasTextVerticalAlign::Middle,
                    line_height: 1.2,
                    wrap: false,
                },
            ),
        ],
        SlideTemplate::Regular => vec![
            template_text_entity(
                CanvasPoint::new(0.0, -220.0),
                CanvasPoint::new(860.0, 80.0),
                "Header",
                HEADER_FONT_SIZE,
                CanvasTextStyle {
                    weight: CanvasTextWeight::Bold,
                    alignment: CanvasTextAlign::Left,
                    vertical_alignment: CanvasTextVerticalAlign::Middle,
                    line_height: 1.2,
                    wrap: false,
                },
            ),
            template_text_entity(
                CanvasPoint::new(0.0, 40.0),
                CanvasPoint::new(860.0, 380.0),
                "Body",
                BODY_FONT_SIZE,
                CanvasTextStyle {
                    weight: CanvasTextWeight::Regular,
                    alignment: CanvasTextAlign::Left,
                    vertical_alignment: CanvasTextVerticalAlign::Top,
                    line_height: 1.3,
                    wrap: true,
                },
            ),
        ],
    }
}

fn slide_frame() -> CanvasEntity {
    let mut frame = CanvasEntity::new(
        CanvasShape::Frame,
        CanvasTransform::new(
            CanvasPoint::default(),
            CanvasPoint::new(DEFAULT_SLIDE_SIZE.x, DEFAULT_SLIDE_SIZE.y),
            0.0,
        ),
    );
    frame.text = "Slide".into();
    frame
}

pub fn build_template_canvas(template: SlideTemplate) -> CanvasContent {
    CanvasContent::new(&Canvas::with_entities(
        std::iter::once(slide_frame()).chain(template_entities(template)),
    ))
}
