use beui_macros::component;

use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::reactive::*;

use crate::frame::{Border, ColorRole};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeKind {
    Dark,
    OledDark,
    Light,
    EInk,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Store)]
pub struct Theme {
    pub kind: ThemeKind,
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
    pub scroll_thumb_hover: Color32,
    pub scroll_thumb_active: Color32,
    pub track: Color32,
    pub knob: Color32,
    pub control_outline: Option<Color32>,
    pub knob_outline: Option<Color32>,
}

impl Theme {
    pub const BUILT_IN: [(&'static str, Theme); 4] = [
        ("Dark", Theme::DARK),
        ("OLED dark", Theme::OLED),
        ("Light", Theme::LIGHT),
        ("E-ink", Theme::EINK),
    ];

    pub const DARK: Theme = Theme {
        kind: ThemeKind::Dark,
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
        scroll_thumb_hover: Color32::from_rgb(84, 98, 124),
        scroll_thumb_active: Color32::from_rgb(110, 127, 158),
        track: Color32::from_rgb(48, 57, 73),
        knob: Color32::from_rgb(226, 232, 244),
        control_outline: None,
        knob_outline: None,
    };

    pub const OLED: Theme = Theme {
        kind: ThemeKind::OledDark,
        background: Color32::BLACK,
        surface: Color32::BLACK,
        surface_raised: Color32::from_rgb(18, 20, 24),
        hover: Color32::from_rgb(24, 27, 33),
        pressed: Color32::from_rgb(36, 41, 50),
        border: Color32::from_rgb(40, 45, 55),
        text: Color32::from_rgb(230, 235, 243),
        text_muted: Color32::from_rgb(141, 153, 174),
        accent: Color32::from_rgb(82, 137, 255),
        accent_hover: Color32::from_rgb(110, 159, 255),
        accent_active: Color32::from_rgb(58, 106, 212),
        accent_soft: Color32::from_rgb(22, 34, 62),
        on_accent: Color32::from_rgb(247, 250, 255),
        danger: Color32::from_rgb(255, 118, 117),
        warning: Color32::from_rgb(247, 190, 86),
        success: Color32::from_rgb(115, 209, 133),
        scroll_thumb: Color32::from_rgb(52, 60, 76),
        scroll_thumb_hover: Color32::from_rgb(78, 90, 114),
        scroll_thumb_active: Color32::from_rgb(110, 127, 158),
        track: Color32::from_rgb(40, 46, 58),
        knob: Color32::from_rgb(226, 232, 244),
        control_outline: None,
        knob_outline: None,
    };

    pub const LIGHT: Theme = Theme {
        kind: ThemeKind::Light,
        background: Color32::from_rgb(243, 245, 248),
        surface: Color32::WHITE,
        surface_raised: Color32::from_rgb(234, 237, 242),
        hover: Color32::from_rgb(228, 232, 238),
        pressed: Color32::from_rgb(214, 219, 228),
        border: Color32::from_rgb(208, 214, 224),
        text: Color32::from_rgb(24, 29, 38),
        text_muted: Color32::from_rgb(92, 102, 120),
        accent: Color32::from_rgb(37, 99, 235),
        accent_hover: Color32::from_rgb(29, 84, 210),
        accent_active: Color32::from_rgb(24, 70, 180),
        accent_soft: Color32::from_rgb(214, 228, 255),
        on_accent: Color32::WHITE,
        danger: Color32::from_rgb(200, 40, 40),
        warning: Color32::from_rgb(170, 110, 0),
        success: Color32::from_rgb(30, 135, 60),
        scroll_thumb: Color32::from_rgb(190, 197, 210),
        scroll_thumb_hover: Color32::from_rgb(160, 169, 186),
        scroll_thumb_active: Color32::from_rgb(130, 141, 162),
        track: Color32::from_rgb(208, 214, 224),
        knob: Color32::WHITE,
        control_outline: None,
        knob_outline: Some(Color32::from_rgb(190, 197, 210)),
    };

    pub const EINK: Theme = Theme {
        kind: ThemeKind::EInk,
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
        scroll_thumb_hover: Color32::from_gray(64),
        scroll_thumb_active: Color32::from_gray(100),
        track: Color32::from_gray(200),
        knob: Color32::WHITE,
        control_outline: Some(Color32::BLACK),
        knob_outline: Some(Color32::BLACK),
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
    let provided = store;
    create_effect(move || store.set(theme.get()));
    provide_context(provided);
    content.call(())
}

pub fn control_border(theme: &ThemeStore) -> Memo<Option<Border>> {
    let outline = theme.control_outline;
    create_memo(move || {
        outline
            .get()
            .map(|color| Border::new(ColorRole::Custom(color)))
    })
}

pub fn knob_border(theme: &ThemeStore) -> Memo<Option<Border>> {
    let outline = theme.knob_outline;
    create_memo(move || {
        outline
            .get()
            .map(|color| Border::new(ColorRole::Custom(color)))
    })
}

pub fn field_border(theme: &ThemeStore, disabled: bool, focused: bool, hovered: bool) -> Color32 {
    if disabled {
        return theme.border.get();
    }
    match (focused, hovered) {
        (true, _) => theme.accent.get(),
        (false, true) => theme.text_muted.get(),
        (false, false) => theme.border.get(),
    }
}

pub fn use_theme() -> ThemeStore {
    use_context::<ThemeStore>().unwrap_or_else(|| with_document(document_theme_store))
}

fn document_theme_store(document: &mut Document) -> ThemeStore {
    document.extension_or_insert_with(|| ThemeStore::new(Theme::DARK))
}

pub trait DocumentTheme {
    fn theme(&self) -> Theme;

    fn set_theme(&mut self, theme: Theme);
}

impl DocumentTheme for Document {
    fn theme(&self) -> Theme {
        self.extension::<ThemeStore>()
            .map_or(Theme::DARK, |store| untrack(|| store.get()))
    }

    fn set_theme(&mut self, theme: Theme) {
        let store = document_theme_store(self);
        store.set(theme);
    }
}

pub const NARROW_WIDTH: f32 = 700.0;

pub const BORDER_WIDTH: f32 = 1.0;
pub const FOCUS_RING_WIDTH: f32 = 2.0;
pub const SEPARATOR_THICKNESS: f32 = 1.0;
pub const SCROLLBAR_TRACK_WIDTH: f32 = 6.0;
pub const SCROLLBAR_INSET: f32 = 2.0;
pub const SCROLLBAR_WIDTH: f32 = SCROLLBAR_TRACK_WIDTH + SCROLLBAR_INSET;
pub const SCROLLBAR_SPACING: f32 = 4.0;
pub const SCROLL_FADE: f32 = 24.0;
