use std::collections::HashMap;

use beui::Image;
use zbus::zvariant::{OwnedValue, Value};

use super::center::Urgency;

const MAX_IMAGE_SIDE: i32 = 1024;

#[derive(Debug, Default, PartialEq)]
pub(crate) struct Hints {
    pub(crate) urgency: Urgency,
    pub(crate) transient: bool,
    pub(crate) resident: bool,
    pub(crate) desktop_entry: Option<String>,
    pub(crate) image: Option<Image>,
    pub(crate) image_path: Option<String>,
}

pub(crate) fn read(hints: &HashMap<String, OwnedValue>) -> Hints {
    let get = |name: &str| hints.get(name).map(|value| unwrap(value));
    let text = |name: &str| match get(name) {
        Some(Value::Str(text)) if !text.is_empty() => Some(text.as_str().to_owned()),
        _ => None,
    };
    let flag = |name: &str| match get(name) {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::U8(flag)) => *flag != 0,
        Some(Value::I32(flag)) => *flag != 0,
        Some(Value::U32(flag)) => *flag != 0,
        _ => false,
    };
    let urgency = match get("urgency") {
        Some(Value::U8(0)) => Urgency::Low,
        Some(Value::U8(2)) => Urgency::Critical,
        _ => Urgency::Normal,
    };
    let image = ["image-data", "image_data", "icon_data"]
        .iter()
        .find_map(|name| get(name).and_then(image_data));
    Hints {
        urgency,
        transient: flag("transient"),
        resident: flag("resident"),
        desktop_entry: text("desktop-entry"),
        image,
        image_path: text("image-path").or_else(|| text("image_path")),
    }
}

fn unwrap<'a>(value: &'a Value<'a>) -> &'a Value<'a> {
    match value {
        Value::Value(inner) => unwrap(inner),
        value => value,
    }
}

pub(crate) fn image_data(value: &Value<'_>) -> Option<Image> {
    let Value::Structure(structure) = unwrap(value) else {
        return None;
    };
    let [width, height, rowstride, alpha, bits, channels, data] = structure.fields() else {
        return None;
    };
    let (
        Value::I32(width),
        Value::I32(height),
        Value::I32(rowstride),
        Value::Bool(alpha),
        Value::I32(bits),
        Value::I32(channels),
        Value::Array(data),
    ) = (width, height, rowstride, alpha, bits, channels, data)
    else {
        return None;
    };
    let fits = |side: i32| (1..=MAX_IMAGE_SIDE).contains(&side);
    let expected = if *alpha { 4 } else { 3 };
    if !fits(*width) || !fits(*height) || *bits != 8 || *channels != expected {
        return None;
    }
    let bytes: Vec<u8> = data
        .inner()
        .iter()
        .map(|byte| match byte {
            Value::U8(byte) => Some(*byte),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let (width, height, rowstride, channels) = (
        usize::try_from(*width).ok()?,
        usize::try_from(*height).ok()?,
        usize::try_from(*rowstride).ok()?,
        usize::try_from(*channels).ok()?,
    );
    if rowstride < width * channels || bytes.len() < rowstride * (height - 1) + width * channels {
        return None;
    }
    let mut pixels = Vec::with_capacity(width * height * 4);
    for row in 0..height {
        for column in 0..width {
            let at = row * rowstride + column * channels;
            pixels.extend_from_slice(&bytes[at..at + 3]);
            pixels.push(if channels == 4 { bytes[at + 3] } else { u8::MAX });
        }
    }
    Some(Image::from_rgba(
        u32::try_from(width).ok()?,
        u32::try_from(height).ok()?,
        pixels,
    ))
}

#[cfg(test)]
mod tests;
