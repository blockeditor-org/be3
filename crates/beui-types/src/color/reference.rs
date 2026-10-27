use std::collections::HashMap;

use beui_tree::reactive::{IntoProp, Memo, Prop, ReadSignal, Signal};
use ui_types::ThemeKind;

use super::Color32;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ColorVar(&'static str);

impl ColorVar {
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    pub const fn name(self) -> &'static str {
        self.0
    }

    pub const fn color(self) -> Color {
        Color::var(self)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ColorScheme {
    #[default]
    Light,
    Dark,
}

impl From<ThemeKind> for ColorScheme {
    fn from(theme: ThemeKind) -> Self {
        match theme {
            ThemeKind::Light | ThemeKind::EPaper => Self::Light,
            ThemeKind::Dark | ThemeKind::OledDark => Self::Dark,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct SchemeColors {
    pub light: Color32,
    pub dark: Color32,
}

impl SchemeColors {
    pub const fn new(light: Color32, dark: Color32) -> Self {
        Self { light, dark }
    }

    pub const fn both(color: Color32) -> Self {
        Self::new(color, color)
    }

    pub const fn get(self, scheme: ColorScheme) -> Color32 {
        match scheme {
            ColorScheme::Light => self.light,
            ColorScheme::Dark => self.dark,
        }
    }
}

pub type ColorDefinitions = HashMap<ColorVar, SchemeColors>;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ColorSource {
    Literal(Color32),
    Var(ColorVar),
}

impl ColorSource {
    fn resolve(self, lookup: &impl Fn(ColorVar) -> Option<Color32>) -> Color32 {
        match self {
            Self::Literal(color) => color,
            Self::Var(var) => lookup(var).unwrap_or(Color32::TRANSPARENT),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Color {
    base: ColorSource,
    mixed: Option<(ColorSource, u8)>,
    alpha: u8,
}

impl Default for Color {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

impl Color {
    pub const TRANSPARENT: Self = Self::literal(Color32::TRANSPARENT);
    pub const BLACK: Self = Self::literal(Color32::BLACK);
    pub const WHITE: Self = Self::literal(Color32::WHITE);

    pub const fn literal(color: Color32) -> Self {
        Self {
            base: ColorSource::Literal(color),
            mixed: None,
            alpha: 255,
        }
    }

    pub const fn var(var: ColorVar) -> Self {
        Self {
            base: ColorSource::Var(var),
            mixed: None,
            alpha: 255,
        }
    }

    pub fn scale_alpha(self, factor: f32) -> Self {
        let alpha = (f32::from(self.alpha) * factor.clamp(0.0, 1.0)).round() as u8;
        Self { alpha, ..self }
    }

    pub fn mix(self, other: Color, share: f32) -> Self {
        debug_assert!(
            self.mixed.is_none() && other.mixed.is_none() && other.alpha == 255,
            "mix takes colours that are not mixed already, and an other colour of full alpha"
        );
        let share = (share.clamp(0.0, 1.0) * 255.0).round() as u8;
        Self {
            mixed: Some((other.base, share)),
            ..self
        }
    }

    pub fn source(self) -> ColorSource {
        self.base
    }

    pub fn mixed(self) -> Option<(ColorSource, f32)> {
        self.mixed
            .map(|(source, share)| (source, f32::from(share) / 255.0))
    }

    pub fn alpha_factor(self) -> f32 {
        f32::from(self.alpha) / 255.0
    }

    pub fn as_literal(self) -> Option<Color32> {
        let literal = |source| match source {
            ColorSource::Literal(color) => Some(color),
            ColorSource::Var(_) => None,
        };
        literal(self.base)?;
        if let Some((source, _)) = self.mixed {
            literal(source)?;
        }
        Some(self.resolve(|_| None))
    }

    pub fn is_clear(self) -> bool {
        self.alpha == 0 || self.as_literal().is_some_and(|color| color.alpha() == 0)
    }

    pub fn describe(self) -> String {
        let source = |source: ColorSource| match source {
            ColorSource::Literal(color) => super::format_hex(color, true),
            ColorSource::Var(var) => format!("--{}", var.name()),
        };
        let mut out = source(self.base);
        if let Some((other, share)) = self.mixed {
            out = format!(
                "{out} mixed {}% with {}",
                u32::from(share) * 100 / 255,
                source(other)
            );
        }
        if self.alpha != 255 {
            out = format!("{out} at {}%", u32::from(self.alpha) * 100 / 255);
        }
        out
    }

    pub fn resolve(self, lookup: impl Fn(ColorVar) -> Option<Color32>) -> Color32 {
        let base = self.base.resolve(&lookup);
        let color = match self.mixed {
            None => base,
            Some((other, share)) => mix(base, other.resolve(&lookup), f32::from(share) / 255.0),
        };
        match self.alpha {
            255 => color,
            alpha => color.scale_alpha(f32::from(alpha) / 255.0),
        }
    }

    pub fn resolve_in(self, definitions: &ColorDefinitions, scheme: ColorScheme) -> Color32 {
        self.resolve(|var| definitions.get(&var).map(|colors| colors.get(scheme)))
    }
}

fn mix(from: Color32, to: Color32, share: f32) -> Color32 {
    let [from, to] = [from, to].map(|color| {
        let [red, green, blue, alpha] = color.to_array().map(f32::from);
        let alpha = alpha / 255.0;
        [red * alpha, green * alpha, blue * alpha, alpha]
    });
    let lerp = |index: usize| from[index] + (to[index] - from[index]) * share;
    let alpha = lerp(3);
    if alpha <= 0.0 {
        return Color32::TRANSPARENT;
    }
    let channel = |index: usize| (lerp(index) / alpha).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(
        channel(0),
        channel(1),
        channel(2),
        (alpha * 255.0).round() as u8,
    )
}

impl From<Color32> for Color {
    fn from(color: Color32) -> Self {
        Self::literal(color)
    }
}

impl From<ColorVar> for Color {
    fn from(var: ColorVar) -> Self {
        Self::var(var)
    }
}

macro_rules! into_color_prop {
    ($target:ty, $value:ty, $convert:expr) => {
        impl IntoProp<$target> for $value {
            fn into_prop(self) -> Prop<$target> {
                Prop::Static(self).map($convert)
            }
        }

        impl IntoProp<$target> for Prop<$value> {
            fn into_prop(self) -> Prop<$target> {
                self.map($convert)
            }
        }

        impl IntoProp<$target> for Memo<$value> {
            fn into_prop(self) -> Prop<$target> {
                IntoProp::<$value>::into_prop(self).map($convert)
            }
        }

        impl IntoProp<$target> for ReadSignal<$value> {
            fn into_prop(self) -> Prop<$target> {
                IntoProp::<$value>::into_prop(self).map($convert)
            }
        }

        impl IntoProp<$target> for Signal<$value> {
            fn into_prop(self) -> Prop<$target> {
                IntoProp::<$value>::into_prop(self).map($convert)
            }
        }
    };
}

into_color_prop!(Color, Color32, Color::literal);
into_color_prop!(Color, ColorVar, Color::var);

impl IntoProp<Option<Color>> for Color32 {
    fn into_prop(self) -> Prop<Option<Color>> {
        Prop::Static(Some(Color::literal(self)))
    }
}
