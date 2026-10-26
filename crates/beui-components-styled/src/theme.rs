use beui_macros::component;

use beui_components_unstyled::SyntaxColors;
use beui_core::color::Color32;
use beui_core::node::NodeId;
use beui_view::reactive::*;

use crate::frame::{Border, ColorRole};

pub use beui_core::preferences::ThemeKind;

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
    pub danger_fill: Color32,
    pub danger_fill_hover: Color32,
    pub danger_fill_active: Color32,
    pub danger_soft: Color32,
    pub on_danger: Color32,
    pub warning: Color32,
    pub success: Color32,
    pub scroll_thumb: Color32,
    pub scroll_thumb_hover: Color32,
    pub scroll_thumb_active: Color32,
    pub track: Color32,
    pub knob: Color32,
    pub control_outline: Option<Color32>,
    pub knob_outline: Option<Color32>,
    pub dim: Color32,
    pub scrim: Color32,
    pub scrim_soft: Color32,
    pub scrim_strong: Color32,
    pub syntax: SyntaxColors,
}

impl Theme {
    pub const fn of(kind: ThemeKind) -> Self {
        match kind {
            ThemeKind::Dark => Self::DARK,
            ThemeKind::OledDark => Self::OLED,
            ThemeKind::Light => Self::LIGHT,
            ThemeKind::EPaper => Self::EPAPER,
        }
    }

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
        danger_fill: Color32::from_rgb(214, 62, 62),
        danger_fill_hover: Color32::from_rgb(230, 84, 84),
        danger_fill_active: Color32::from_rgb(184, 46, 46),
        danger_soft: Color32::from_rgb(72, 32, 36),
        on_danger: Color32::from_rgb(255, 247, 247),
        warning: Color32::from_rgb(247, 190, 86),
        success: Color32::from_rgb(115, 209, 133),
        scroll_thumb: Color32::from_rgb(60, 71, 92),
        scroll_thumb_hover: Color32::from_rgb(84, 98, 124),
        scroll_thumb_active: Color32::from_rgb(110, 127, 158),
        track: Color32::from_rgb(48, 57, 73),
        knob: Color32::from_rgb(226, 232, 244),
        control_outline: None,
        knob_outline: None,
        dim: Color32::from_rgba_unmultiplied(0, 0, 0, 96),
        scrim: Color32::from_rgba_unmultiplied(0, 0, 0, 150),
        scrim_soft: Color32::from_rgba_unmultiplied(0, 0, 0, 90),
        scrim_strong: Color32::from_rgba_unmultiplied(0, 0, 0, 230),
        syntax: DARK_SYNTAX,
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
        danger_fill: Color32::from_rgb(214, 62, 62),
        danger_fill_hover: Color32::from_rgb(230, 84, 84),
        danger_fill_active: Color32::from_rgb(184, 46, 46),
        danger_soft: Color32::from_rgb(56, 22, 26),
        on_danger: Color32::from_rgb(255, 247, 247),
        warning: Color32::from_rgb(247, 190, 86),
        success: Color32::from_rgb(115, 209, 133),
        scroll_thumb: Color32::from_rgb(52, 60, 76),
        scroll_thumb_hover: Color32::from_rgb(78, 90, 114),
        scroll_thumb_active: Color32::from_rgb(110, 127, 158),
        track: Color32::from_rgb(40, 46, 58),
        knob: Color32::from_rgb(226, 232, 244),
        control_outline: None,
        knob_outline: None,
        dim: Color32::from_rgba_unmultiplied(0, 0, 0, 96),
        scrim: Color32::from_rgba_unmultiplied(0, 0, 0, 150),
        scrim_soft: Color32::from_rgba_unmultiplied(0, 0, 0, 90),
        scrim_strong: Color32::from_rgba_unmultiplied(0, 0, 0, 230),
        syntax: DARK_SYNTAX,
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
        danger_fill: Color32::from_rgb(200, 40, 40),
        danger_fill_hover: Color32::from_rgb(178, 32, 32),
        danger_fill_active: Color32::from_rgb(156, 26, 26),
        danger_soft: Color32::from_rgb(248, 214, 214),
        on_danger: Color32::WHITE,
        warning: Color32::from_rgb(170, 110, 0),
        success: Color32::from_rgb(30, 135, 60),
        scroll_thumb: Color32::from_rgb(190, 197, 210),
        scroll_thumb_hover: Color32::from_rgb(160, 169, 186),
        scroll_thumb_active: Color32::from_rgb(130, 141, 162),
        track: Color32::from_rgb(208, 214, 224),
        knob: Color32::WHITE,
        control_outline: None,
        knob_outline: Some(Color32::from_rgb(190, 197, 210)),
        dim: Color32::from_rgba_unmultiplied(16, 24, 40, 40),
        scrim: Color32::from_rgba_unmultiplied(0, 0, 0, 150),
        scrim_soft: Color32::from_rgba_unmultiplied(0, 0, 0, 90),
        scrim_strong: Color32::from_rgba_unmultiplied(0, 0, 0, 230),
        syntax: LIGHT_SYNTAX,
    };

    pub const EPAPER: Theme = Theme {
        kind: ThemeKind::EPaper,
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
        danger_fill: Color32::BLACK,
        danger_fill_hover: Color32::from_gray(64),
        danger_fill_active: Color32::from_gray(100),
        danger_soft: Color32::from_gray(212),
        on_danger: Color32::WHITE,
        warning: Color32::from_gray(80),
        success: Color32::from_gray(80),
        scroll_thumb: Color32::BLACK,
        scroll_thumb_hover: Color32::from_gray(64),
        scroll_thumb_active: Color32::from_gray(100),
        track: Color32::from_gray(200),
        knob: Color32::WHITE,
        control_outline: Some(Color32::BLACK),
        knob_outline: Some(Color32::BLACK),
        dim: Color32::TRANSPARENT,
        scrim: Color32::from_rgba_unmultiplied(0, 0, 0, 150),
        scrim_soft: Color32::from_rgba_unmultiplied(0, 0, 0, 90),
        scrim_strong: Color32::from_rgba_unmultiplied(0, 0, 0, 230),
        syntax: EPAPER_SYNTAX,
    };
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const DARK_SYNTAX: SyntaxColors = SyntaxColors {
    invalid: rgb(0xff0000),
    keyword: rgb(0x5ec4ff),
    keyword_storage: rgb(0x008b94),
    keyword_primitive_type: rgb(0x70e1e8),
    literal: rgb(0xe27e8d),
    literal_string: rgb(0x68a1f0),
    comment: rgb(0xff9d1c),
    punctuation: rgb(0x718ca1),
    punctuation_important: rgb(0xb7c5d3),
    variable: rgb(0x718ca1),
    variable_constant: rgb(0x8bd49c),
    variable_function: rgb(0x70e1e8),
    variable_mutable: rgb(0xb7c5d3),
    variable_parameter: rgb(0xebbf83),
    markdown_plain_text: rgb(0xffffff),
    markdown_symbol: rgb(0x5a6579),
    markdown_link: rgb(0x7aa5ff),
    markdown_code: rgb(0x8bd49c),
    unstyled: rgb(0xb7c5d3),
    invisible: rgb(0x43515c),
};

const LIGHT_SYNTAX: SyntaxColors = SyntaxColors {
    invalid: rgb(0xd1242f),
    keyword: rgb(0xcf222e),
    keyword_storage: rgb(0x953800),
    keyword_primitive_type: rgb(0x0550ae),
    literal: rgb(0x0550ae),
    literal_string: rgb(0x0a3069),
    comment: rgb(0x6e7781),
    punctuation: rgb(0x57606a),
    punctuation_important: rgb(0x24292f),
    variable: rgb(0x57606a),
    variable_constant: rgb(0x116329),
    variable_function: rgb(0x8250df),
    variable_mutable: rgb(0x24292f),
    variable_parameter: rgb(0x953800),
    markdown_plain_text: rgb(0x181d26),
    markdown_symbol: rgb(0x9ea8b7),
    markdown_link: rgb(0x1d54d2),
    markdown_code: rgb(0x116329),
    unstyled: rgb(0x24292f),
    invisible: rgb(0xc0c6cf),
};

const EPAPER_SYNTAX: SyntaxColors = SyntaxColors {
    comment: Color32::from_gray(80),
    punctuation: Color32::from_gray(80),
    markdown_symbol: Color32::from_gray(80),
    invisible: Color32::from_gray(180),
    ..SyntaxColors::uniform(Color32::BLACK)
};

impl Default for Theme {
    fn default() -> Self {
        Self::DARK
    }
}

#[component]
pub fn ThemeProvider(theme: Prop<Theme>, #[prop(children)] content: Render) -> NodeId {
    let store = ThemeStore::new(theme.get_untracked());
    create_effect(move || store.set(theme.get()));
    provide_context(store);
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
    use_context::<ThemeStore>().unwrap_or_else(tree_theme_store)
}

fn tree_theme_store() -> ThemeStore {
    let preferences = use_preferences();
    let kind = create_memo(move || preferences.get().theme);
    tree_local(|| {
        let store = ThemeStore::new(Theme::of(kind.get_untracked()));
        create_effect(move || store.set(Theme::of(kind.get())));
        store
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Density {
    #[default]
    Compact,
    Comfortable,
}

#[derive(Clone, Copy)]
struct DensityContext(Memo<Density>);

pub fn provide_density(density: Memo<Density>) {
    provide_context(DensityContext(density));
}

pub fn use_density() -> Memo<Density> {
    match use_context::<DensityContext>() {
        Some(DensityContext(density)) => density,
        None => create_memo(|| Density::Compact),
    }
}

pub const NARROW_WIDTH: f32 = 700.0;

pub const BORDER_WIDTH: f32 = 1.0;
pub const FOCUS_RING_WIDTH: f32 = 2.0;
pub const SEPARATOR_THICKNESS: f32 = 1.0;
pub const POINTER_REACH: f32 = 10.0;
pub const SCROLLBAR_TRACK_WIDTH: f32 = 6.0;
pub const SCROLLBAR_INSET: f32 = 2.0;
pub const SCROLLBAR_WIDTH: f32 = SCROLLBAR_TRACK_WIDTH + SCROLLBAR_INSET;
pub const SCROLL_FADE: f32 = 24.0;
