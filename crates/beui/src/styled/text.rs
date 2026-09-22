use crate::color::Color32;

use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::font::line_height;
use crate::node::NodeId;
use crate::reactive::{IntoProp, Prop, Text, clone, create_memo};
use crate::styled::theme::{
    FONT_BODY, FONT_DISPLAY, FONT_HEADING, FONT_SMALL, FONT_TITLE, icon_glyph_size, icon_text_size,
    use_theme,
};

fn text_color() -> Prop<Color32> {
    use_theme().text.clone().into_prop()
}

fn muted_color() -> Prop<Color32> {
    use_theme().text_muted.clone().into_prop()
}

#[component]
pub fn Code(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = text_color())] color: Prop<Color32>,
) -> NodeId {
    view! {
        <Text string={content} font_size=FONT_SMALL color align monospace=true />
    }
}

#[component]
pub fn Icon(
    glyph: Prop<String>,
    #[prop(default = FONT_BODY)] text_size: Prop<f32>,
    #[prop(default = text_color())] color: Prop<Color32>,
) -> NodeId {
    let font_size = create_memo(clone!(text_size -> move || icon_glyph_size(text_size.get())));
    let line = create_memo(move || line_height(text_size.get()));
    view! {
        <IconGlyph glyph font_size line_height={line} color />
    }
}

#[component]
pub fn IconSized(glyph: Prop<String>, font_size: Prop<f32>, color: Prop<Color32>) -> NodeId {
    let line = create_memo(clone!(font_size -> move || {
        line_height(icon_text_size(font_size.get()))
    }));
    view! {
        <IconGlyph glyph font_size line_height={line} color />
    }
}

#[component]
fn IconGlyph(
    glyph: Prop<String>,
    font_size: Prop<f32>,
    line_height: Prop<f32>,
    color: Prop<Color32>,
) -> NodeId {
    view! {
        <Text string={glyph} font_size line_height color align=TextAlign::Center icon=true />
    }
}

#[component]
fn Line(
    content: Prop<String>,
    font_size: Prop<f32>,
    color: Prop<Color32>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = false)] wrap: Prop<bool>,
) -> NodeId {
    let text = create_memo(move || content.get());
    view! {
        <Text string={text} font_size color align wrap />
    }
}

#[component]
pub fn Display(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = text_color())] color: Prop<Color32>,
) -> NodeId {
    view! {
        <Line content font_size=FONT_DISPLAY color align />
    }
}

#[component]
pub fn Title(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = text_color())] color: Prop<Color32>,
) -> NodeId {
    view! {
        <Line content font_size=FONT_TITLE color align />
    }
}

#[component]
pub fn Heading(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = text_color())] color: Prop<Color32>,
) -> NodeId {
    view! {
        <Line content font_size=FONT_HEADING color align />
    }
}

#[component]
pub fn Body(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = text_color())] color: Prop<Color32>,
) -> NodeId {
    view! {
        <Line content font_size=FONT_BODY color align />
    }
}

#[component]
pub fn Caption(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = muted_color())] color: Prop<Color32>,
    #[prop(default = false)] wrap: Prop<bool>,
) -> NodeId {
    view! {
        <Line content font_size=FONT_SMALL color align wrap />
    }
}

#[component]
pub fn Paragraph(
    content: Prop<String>,
    #[prop(default = TextAlign::Start)] align: Prop<TextAlign>,
    #[prop(default = muted_color())] color: Prop<Color32>,
) -> NodeId {
    let text = create_memo(move || content.get());
    view! {
        <Text string={text} font_size=FONT_BODY color align wrap=true />
    }
}
