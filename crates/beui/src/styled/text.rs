use crate::color::Color32;

use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::node::NodeId;
use crate::reactive::{IntoProp, Prop, Text, create_memo};
use crate::styled::theme::{
    FONT_BODY, FONT_DISPLAY, FONT_HEADING, FONT_SMALL, FONT_TITLE, ICON_SIZE, use_theme,
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
pub fn Icon(glyph: Prop<String>, #[prop(default = text_color())] color: Prop<Color32>) -> NodeId {
    view! {
        <IconSized glyph font_size=ICON_SIZE color />
    }
}

#[component]
pub fn IconSized(glyph: Prop<String>, font_size: Prop<f32>, color: Prop<Color32>) -> NodeId {
    view! {
        <Text string={glyph} font_size color align=TextAlign::Center icon=true />
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
    #[prop(default = muted_color())] color: Prop<Color32>,
) -> NodeId {
    let text = create_memo(move || content.get());
    view! {
        <Text string={text} font_size=FONT_BODY color wrap=true />
    }
}
