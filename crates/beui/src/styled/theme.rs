use beui_macros::component;

use crate::color::Color32;
use crate::node::NodeId;
use crate::reactive::{
    Memo, Prop, Render, Store, create_effect, create_memo, provide_context, use_context,
    with_document,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Store)]
pub struct Theme {
    pub background: Color32,
    pub surface: Color32,
    pub surface_raised: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_active: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub danger: Color32,
    pub warning: Color32,
    pub success: Color32,
    pub scroll_thumb: Color32,
    pub track: Color32,
    pub knob: Color32,
    pub control_outline: Option<Color32>,
}

impl Theme {
    pub const DARK: Theme = Theme {
        background: Color32::from_rgb(14, 17, 23),
        surface: Color32::from_rgb(22, 26, 34),
        surface_raised: Color32::from_rgb(31, 37, 48),
        hover: Color32::from_rgb(31, 37, 48),
        pressed: Color32::from_rgb(42, 50, 64),
        border: Color32::from_rgb(42, 50, 64),
        text: Color32::from_rgb(230, 235, 243),
        text_muted: Color32::from_rgb(141, 153, 174),
        accent: Color32::from_rgb(82, 137, 255),
        accent_hover: Color32::from_rgb(110, 159, 255),
        accent_active: Color32::from_rgb(58, 106, 212),
        accent_soft: Color32::from_rgb(33, 48, 84),
        on_accent: Color32::from_rgb(247, 250, 255),
        danger: Color32::from_rgb(255, 118, 117),
        warning: Color32::from_rgb(247, 190, 86),
        success: Color32::from_rgb(115, 209, 133),
        scroll_thumb: Color32::from_rgb(60, 71, 92),
        track: Color32::from_rgb(48, 57, 73),
        knob: Color32::from_rgb(226, 232, 244),
        control_outline: None,
    };

    pub const EINK: Theme = Theme {
        background: Color32::WHITE,
        surface: Color32::WHITE,
        surface_raised: Color32::WHITE,
        hover: Color32::from_gray(232),
        pressed: Color32::from_gray(200),
        border: Color32::BLACK,
        text: Color32::BLACK,
        text_muted: Color32::from_gray(80),
        accent: Color32::BLACK,
        accent_hover: Color32::from_gray(64),
        accent_active: Color32::from_gray(100),
        accent_soft: Color32::from_gray(212),
        on_accent: Color32::WHITE,
        danger: Color32::BLACK,
        warning: Color32::from_gray(80),
        success: Color32::from_gray(80),
        scroll_thumb: Color32::BLACK,
        track: Color32::from_gray(200),
        knob: Color32::WHITE,
        control_outline: Some(Color32::BLACK),
    };
}

impl Default for Theme {
    fn default() -> Self {
        Self::DARK
    }
}

#[component]
pub fn ThemeProvider(theme: Prop<Theme>, #[prop(children)] content: Render) -> NodeId {
    let store = ThemeStore::new(theme.peek());
    let provided = store.clone();
    create_effect(move || store.set(theme.get()));
    provide_context(provided);
    content.call(())
}

pub fn control_outline(theme: &ThemeStore) -> Memo<Color32> {
    let outline = theme.control_outline.clone();
    create_memo(move || outline.get().unwrap_or(Color32::TRANSPARENT))
}

pub fn control_outline_visible(theme: &ThemeStore) -> Memo<bool> {
    let outline = theme.control_outline.clone();
    create_memo(move || outline.get().is_some())
}

pub fn use_theme() -> ThemeStore {
    use_context::<ThemeStore>().unwrap_or_else(|| with_document(|document| document.theme_store()))
}

pub const FONT_SMALL: f32 = 12.0;
pub const FONT_BODY: f32 = 14.0;
pub const FONT_HEADING: f32 = 16.0;
pub const FONT_TITLE: f32 = 21.0;
pub const FONT_DISPLAY: f32 = 46.0;
pub const ICON_SIZE: f32 = 18.0;

pub const RADIUS: u8 = 6;
pub const CARD_RADIUS: u8 = 10;
pub const CHIP_RADIUS: u8 = 4;

pub const NARROW_WIDTH: f32 = 720.0;

pub const BORDER_WIDTH: f32 = 1.0;
pub const SEPARATOR_HEIGHT: f32 = 1.0;
pub const SCROLLBAR_WIDTH: f32 = 6.0;
pub const SCROLLBAR_SPACING: f32 = 4.0;
