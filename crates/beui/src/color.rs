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
