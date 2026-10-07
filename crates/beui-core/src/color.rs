#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Color32([u8; 4]);

impl Color32 {
    pub const TRANSPARENT: Self = Self::from_rgba_unmultiplied(0, 0, 0, 0);
    pub const BLACK: Self = Self::from_rgb(0, 0, 0);
    pub const WHITE: Self = Self::from_rgb(255, 255, 255);

    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self([red, green, blue, 255])
    }

    pub const fn from_rgba_unmultiplied(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self([red, green, blue, alpha])
    }

    pub const fn from_gray(level: u8) -> Self {
        Self([level, level, level, 255])
    }

    pub const fn to_array(self) -> [u8; 4] {
        self.0
    }

    pub const fn alpha(self) -> u8 {
        self.0[3]
    }

    pub fn scale_alpha(self, factor: f32) -> Self {
        let [red, green, blue, alpha] = self.0;
        let alpha = (f32::from(alpha) * factor.clamp(0.0, 1.0)).round() as u8;
        Self([red, green, blue, alpha])
    }

    pub fn to_normalized_gamma_f32(self) -> [f32; 4] {
        let [red, green, blue, alpha] = self.0;
        [
            red as f32 / 255.0,
            green as f32 / 255.0,
            blue as f32 / 255.0,
            alpha as f32 / 255.0,
        ]
    }

    pub fn to_linear_f32(self) -> [f32; 4] {
        let [red, green, blue, alpha] = self.0;
        [
            linear_from_gamma(red),
            linear_from_gamma(green),
            linear_from_gamma(blue),
            alpha as f32 / 255.0,
        ]
    }
}

fn linear_from_gamma(value: u8) -> f32 {
    let value = value as f32 / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Hsva {
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
    pub alpha: f32,
}

impl Hsva {
    pub fn new(hue: f32, saturation: f32, value: f32, alpha: f32) -> Self {
        Self {
            hue: hue.rem_euclid(360.0),
            saturation: saturation.clamp(0.0, 1.0),
            value: value.clamp(0.0, 1.0),
            alpha: alpha.clamp(0.0, 1.0),
        }
    }

    pub fn from_color(color: Color32) -> Self {
        let [red, green, blue, alpha] = color.to_normalized_gamma_f32();
        let max = red.max(green).max(blue);
        let min = red.min(green).min(blue);
        let chroma = max - min;
        let hue = if chroma <= 0.0 {
            0.0
        } else if max == red {
            60.0 * ((green - blue) / chroma).rem_euclid(6.0)
        } else if max == green {
            60.0 * ((blue - red) / chroma + 2.0)
        } else {
            60.0 * ((red - green) / chroma + 4.0)
        };
        let saturation = if max <= 0.0 { 0.0 } else { chroma / max };
        Self::new(hue, saturation, max, alpha)
    }

    pub fn from_color_keeping(color: Color32, previous: Hsva) -> Self {
        if previous.to_color() == color {
            return previous;
        }
        let mut next = Self::from_color(color);
        let [red, green, blue, _] = color.to_array();
        if red == green && green == blue {
            next.hue = previous.hue;
            if red == 0 {
                next.saturation = previous.saturation;
            }
        }
        next
    }

    pub fn to_color(self) -> Color32 {
        let [red, green, blue] = self.rgb();
        let channel = |value: f32| (value * 255.0).round().clamp(0.0, 255.0) as u8;
        Color32::from_rgba_unmultiplied(
            channel(red),
            channel(green),
            channel(blue),
            channel(self.alpha),
        )
    }

    pub fn opaque(self) -> Self {
        Self { alpha: 1.0, ..self }
    }

    pub fn rgb(self) -> [f32; 3] {
        let hue = self.hue.rem_euclid(360.0) / 60.0;
        let chroma = self.value * self.saturation;
        let second = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
        let (red, green, blue) = match hue as u32 {
            0 => (chroma, second, 0.0),
            1 => (second, chroma, 0.0),
            2 => (0.0, chroma, second),
            3 => (0.0, second, chroma),
            4 => (second, 0.0, chroma),
            _ => (chroma, 0.0, second),
        };
        let lift = self.value - chroma;
        [red + lift, green + lift, blue + lift]
    }
}

const OKLCH_CHROMA_LIMIT: f32 = 0.5;
const OKLCH_SEARCH_STEPS: u32 = 24;
const CUSP_SEARCH_STEPS: u32 = 32;
const GAMUT_SLACK: f32 = 1e-4;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Oklch {
    pub lightness: f32,
    pub chroma: f32,
    pub hue: f32,
    pub alpha: f32,
}

impl Oklch {
    pub fn new(lightness: f32, chroma: f32, hue: f32, alpha: f32) -> Self {
        Self {
            lightness: lightness.clamp(0.0, 1.0),
            chroma: chroma.max(0.0),
            hue: hue.rem_euclid(360.0),
            alpha: alpha.clamp(0.0, 1.0),
        }
    }

    pub fn from_color(color: Color32) -> Self {
        let [red, green, blue, alpha] = color.to_linear_f32();
        let long = (0.412_221_46 * red + 0.536_332_55 * green + 0.051_445_99 * blue).cbrt();
        let medium = (0.211_903_5 * red + 0.680_699_5 * green + 0.107_396_96 * blue).cbrt();
        let short = (0.088_302_46 * red + 0.281_718_85 * green + 0.629_978_7 * blue).cbrt();
        let lightness = 0.210_454_26 * long + 0.793_617_8 * medium - 0.004_072_047 * short;
        let a = 1.977_998_5 * long - 2.428_592_2 * medium + 0.450_593_7 * short;
        let b = 0.025_904_037 * long + 0.782_771_77 * medium - 0.808_675_77 * short;
        let chroma = a.hypot(b);
        let hue = match chroma < GAMUT_SLACK {
            true => 0.0,
            false => b.atan2(a).to_degrees(),
        };
        Self::new(lightness, chroma, hue, alpha)
    }

    pub fn from_color_keeping(color: Color32, previous: Oklch) -> Self {
        if previous.to_color() == color {
            return previous;
        }
        let mut next = Self::from_color(color);
        let [red, green, blue, _] = color.to_array();
        if red == green && green == blue {
            next.hue = previous.hue;
        }
        next
    }

    pub fn max_chroma(lightness: f32, hue: f32) -> f32 {
        let (mut inside, mut outside) = (0.0, OKLCH_CHROMA_LIMIT);
        for _ in 0..OKLCH_SEARCH_STEPS {
            let middle = (inside + outside) / 2.0;
            match Self::new(lightness, middle, hue, 1.0).in_gamut() {
                true => inside = middle,
                false => outside = middle,
            }
        }
        inside
    }

    pub fn cusp(hue: f32) -> Self {
        let (mut darker, mut lighter) = (0.0, 1.0);
        for _ in 0..CUSP_SEARCH_STEPS {
            let third = (lighter - darker) / 3.0;
            let (low, high) = (darker + third, lighter - third);
            match Self::max_chroma(low, hue) < Self::max_chroma(high, hue) {
                true => darker = low,
                false => lighter = high,
            }
        }
        let lightness = (darker + lighter) / 2.0;
        Self::new(lightness, Self::max_chroma(lightness, hue), hue, 1.0)
    }

    pub fn in_gamut(self) -> bool {
        self.linear_rgb()
            .iter()
            .all(|channel| (-GAMUT_SLACK..=1.0 + GAMUT_SLACK).contains(channel))
    }

    pub fn clamped(self) -> Self {
        match self.in_gamut() {
            true => self,
            false => Self {
                chroma: Self::max_chroma(self.lightness, self.hue),
                ..self
            },
        }
    }

    pub fn opaque(self) -> Self {
        Self { alpha: 1.0, ..self }
    }

    pub fn to_color(self) -> Color32 {
        let [red, green, blue] = self.clamped().linear_rgb();
        let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_rgba_unmultiplied(
            gamma_from_linear(red),
            gamma_from_linear(green),
            gamma_from_linear(blue),
            channel(self.alpha),
        )
    }

    pub fn linear_rgb(self) -> [f32; 3] {
        let (sine, cosine) = self.hue.to_radians().sin_cos();
        let (a, b) = (self.chroma * cosine, self.chroma * sine);
        let long = (self.lightness + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
        let medium = (self.lightness - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
        let short = (self.lightness - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
        [
            4.076_741_7 * long - 3.307_711_6 * medium + 0.230_969_94 * short,
            -1.268_438 * long + 2.609_757_4 * medium - 0.341_319_38 * short,
            -0.004_196_086_3 * long - 0.703_418_6 * medium + 1.707_614_7 * short,
        ]
    }
}

pub fn gamma_from_linear(value: f32) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let encoded = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round().clamp(0.0, 255.0) as u8
}

pub fn format_hex(color: Color32, alpha: bool) -> String {
    let [red, green, blue, opacity] = color.to_array();
    match alpha {
        true => format!("#{red:02X}{green:02X}{blue:02X}{opacity:02X}"),
        false => format!("#{red:02X}{green:02X}{blue:02X}"),
    }
}

pub fn parse_hex(text: &str) -> Option<Color32> {
    let text = text.trim();
    let text = text.strip_prefix('#').unwrap_or(text);
    if !text.is_ascii() || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |start: usize| u8::from_str_radix(&text[start..start + 2], 16).ok();
    let short = |index: usize| {
        u8::from_str_radix(&text[index..=index], 16)
            .ok()
            .map(|v| v * 17)
    };
    match text.len() {
        3 => Some(Color32::from_rgb(short(0)?, short(1)?, short(2)?)),
        4 => Some(Color32::from_rgba_unmultiplied(
            short(0)?,
            short(1)?,
            short(2)?,
            short(3)?,
        )),
        6 => Some(Color32::from_rgb(channel(0)?, channel(2)?, channel(4)?)),
        8 => Some(Color32::from_rgba_unmultiplied(
            channel(0)?,
            channel(2)?,
            channel(4)?,
            channel(6)?,
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
