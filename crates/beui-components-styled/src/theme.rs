use beui_macros::component;

use beui_components_unstyled::SyntaxColors;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{Color, Color32, ColorDefinitions, ColorVar, SchemeColors};

use crate::frame::{Border, ColorRole};
use crate::tokens::{Size, Space};

pub use beui_view::types::ThemeKind;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    pub accent_soft_hover: Color32,
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
        accent_soft_hover: Color32::from_rgb(44, 63, 108),
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
        accent_soft_hover: Color32::from_rgb(32, 48, 86),
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
        accent_soft_hover: Color32::from_rgb(196, 214, 252),
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
        accent_soft_hover: Color32::from_gray(192),
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

const fn rgb(hex: u32) -> Color {
    Color::literal(Color32::from_rgb(
        (hex >> 16) as u8,
        (hex >> 8) as u8,
        hex as u8,
    ))
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
    comment: Color::literal(Color32::from_gray(80)),
    punctuation: Color::literal(Color32::from_gray(80)),
    markdown_symbol: Color::literal(Color32::from_gray(80)),
    invisible: Color::literal(Color32::from_gray(180)),
    ..SyntaxColors::uniform(Color::BLACK)
};

macro_rules! color_roles {
    (roles: [$($role:ident),* $(,)?], outlines: [$($outline:ident),* $(,)?], syntax: [$($syntax:ident),* $(,)?] $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Store)]
        pub struct ThemeTokens {
            pub kind: ThemeKind,
            $(pub $role: Color,)*
            $(pub $outline: Option<Color>,)*
            pub syntax: SyntaxColors,
        }

        impl ThemeTokens {
            pub fn literal(theme: &Theme) -> Self {
                Self {
                    kind: theme.kind,
                    $($role: theme.$role.into(),)*
                    $($outline: theme.$outline.map(Color::from),)*
                    syntax: theme.syntax,
                }
            }

            pub fn resolve(&self) -> Theme {
                Theme {
                    kind: self.kind,
                    $($role: resolve_color(self.$role),)*
                    $($outline: self.$outline.map(resolve_color),)*
                    syntax: self.syntax,
                }
            }

            pub fn referenced(kind: ThemeKind, light: &Theme, dark: &Theme) -> Self {
                Self {
                    kind,
                    $($role: ColorVar::new(concat!("color-", stringify!($role))).color(),)*
                    $($outline: (light.$outline.is_some() || dark.$outline.is_some())
                        .then(|| ColorVar::new(concat!("color-", stringify!($outline))).color()),)*
                    syntax: SyntaxColors {
                        $($syntax: ColorVar::new(concat!("syntax-", stringify!($syntax))).color(),)*
                    },
                }
            }
        }

        impl Theme {
            pub fn definitions(light: &Theme, dark: &Theme) -> ColorDefinitions {
                let syntax = |color: Color| color.as_literal().unwrap_or(Color32::TRANSPARENT);
                let outline = |color: Option<Color32>| color.unwrap_or(Color32::TRANSPARENT);
                ColorDefinitions::from([
                    $((
                        ColorVar::new(concat!("color-", stringify!($role))),
                        SchemeColors::new(light.$role, dark.$role),
                    ),)*
                    $((
                        ColorVar::new(concat!("color-", stringify!($outline))),
                        SchemeColors::new(outline(light.$outline), outline(dark.$outline)),
                    ),)*
                    $((
                        ColorVar::new(concat!("syntax-", stringify!($syntax))),
                        SchemeColors::new(syntax(light.syntax.$syntax), syntax(dark.syntax.$syntax)),
                    ),)*
                ])
            }
        }
    };
}

pub type ThemeStore = ThemeTokensStore;

color_roles! {
    roles: [
        background, surface, surface_raised, hover, pressed, border, text, text_muted, accent,
        accent_hover, accent_active, accent_soft, accent_soft_hover, on_accent, danger, danger_fill,
        danger_fill_hover, danger_fill_active, danger_soft, on_danger, warning, success,
        scroll_thumb, scroll_thumb_hover, scroll_thumb_active, track, knob, dim, scrim, scrim_soft,
        scrim_strong,
    ],
    outlines: [control_outline, knob_outline],
    syntax: [
        invalid, keyword, keyword_storage, keyword_primitive_type, literal, literal_string, comment,
        punctuation, punctuation_important, variable, variable_constant, variable_function,
        variable_mutable, variable_parameter, markdown_plain_text, markdown_symbol, markdown_link,
        markdown_code, unstyled, invisible,
    ],
}

impl Default for Theme {
    fn default() -> Self {
        Self::DARK
    }
}

#[component]
pub fn ThemeProvider(theme: Prop<Theme>, #[prop(children)] content: Render) -> NodeId {
    let store = ThemeStore::new(ThemeTokens::literal(&theme.get_untracked()));
    create_effect(move || store.set(ThemeTokens::literal(&theme.get())));
    provide_context(store);
    content.call(())
}

pub fn control_border(theme: &ThemeStore) -> Memo<Option<Border>> {
    let outline = theme.control_outline;
    create_memo(move || {
        outline
            .get()
            .map(|_| Border::new(ColorRole::ControlOutline))
    })
}

pub fn knob_border(theme: &ThemeStore) -> Memo<Option<Border>> {
    let outline = theme.knob_outline;
    create_memo(move || outline.get().map(|_| Border::new(ColorRole::KnobOutline)))
}

pub fn field_border(disabled: bool, focused: bool, hovered: bool) -> ColorRole {
    if disabled {
        return ColorRole::Border;
    }
    match (focused, hovered) {
        (true, _) => ColorRole::Accent,
        (false, true) => ColorRole::TextMuted,
        (false, false) => ColorRole::Border,
    }
}

pub fn use_theme() -> ThemeStore {
    use_context::<ThemeStore>().unwrap_or_else(tree_theme_store)
}

fn scheme_themes(kind: ThemeKind) -> (Theme, Theme) {
    let light = match kind {
        ThemeKind::EPaper => Theme::EPAPER,
        _ => Theme::LIGHT,
    };
    let dark = match kind {
        ThemeKind::OledDark => Theme::OLED,
        _ => Theme::DARK,
    };
    (light, dark)
}

fn tree_theme_store() -> ThemeStore {
    let preferences = use_preferences();
    let kind = create_memo(move || preferences.get().theme);
    tree_local(|| {
        let themes = create_memo(move || scheme_themes(kind.get()));
        define_colors(create_memo(move || {
            let (light, dark) = themes.get();
            Theme::definitions(&light, &dark)
        }));
        let referenced = move || {
            let (light, dark) = themes.get();
            ThemeTokens::referenced(kind.get(), &light, &dark)
        };
        let store = ThemeStore::new(untrack(referenced));
        create_effect(move || store.set(referenced()));
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
    density_context().unwrap_or_else(|| create_memo(|| Density::Compact))
}

pub(crate) fn density_context() -> Option<Memo<Density>> {
    use_context::<DensityContext>().map(|DensityContext(density)| density)
}

pub const NARROW_WIDTH: f32 = 700.0;

pub const POINTER_REACH: f32 = 10.0;
pub const SCROLLBAR_WIDTH: f32 = Size::Track.px() + Space::Xs2.px();
pub const SCROLL_FADE: f32 = 24.0;
