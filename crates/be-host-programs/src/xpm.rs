use std::collections::HashMap;

const MAX_SIDE: u32 = 1024;
const MAX_COLORS: usize = 1 << 16;
const MAX_CHARS_PER_PIXEL: usize = 4;
const KEYS: [&str; 5] = ["c", "g", "g4", "m", "s"];
const PREFERRED: [&str; 4] = ["c", "g", "g4", "m"];

pub fn decode(data: &[u8]) -> Option<image::RgbaImage> {
    let text = std::str::from_utf8(data).ok()?;
    if !text.trim_start().starts_with("/* XPM */") {
        return None;
    }
    let strings = strings(text);
    let mut strings = strings.iter();
    let mut header = strings.next()?.split_whitespace().map(str::parse::<usize>);
    let width = header.next()?.ok()?;
    let height = header.next()?.ok()?;
    let colors = header.next()?.ok()?;
    let per_pixel = header.next()?.ok()?;
    if width == 0
        || height == 0
        || width > MAX_SIDE as usize
        || height > MAX_SIDE as usize
        || colors == 0
        || colors > MAX_COLORS
        || per_pixel == 0
        || per_pixel > MAX_CHARS_PER_PIXEL
    {
        return None;
    }
    let mut table = HashMap::with_capacity(colors);
    for _ in 0..colors {
        let line = strings.next()?;
        let key = line.get(..per_pixel)?;
        table.insert(key.to_owned(), color(line.get(per_pixel..)?));
    }
    let mut rgba = Vec::with_capacity(width * height * 4);
    for _ in 0..height {
        let row = strings.next()?;
        for column in 0..width {
            let key = row.get(column * per_pixel..(column + 1) * per_pixel)?;
            rgba.extend(table.get(key).copied().unwrap_or([0, 0, 0, 0]));
        }
    }
    image::RgbaImage::from_raw(width as u32, height as u32, rgba)
}

fn strings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(next) = chars.next() {
        match next {
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut last = ' ';
                for inside in chars.by_ref() {
                    if last == '*' && inside == '/' {
                        break;
                    }
                    last = inside;
                }
            }
            '"' => {
                let mut string = String::new();
                while let Some(inside) = chars.next() {
                    match inside {
                        '"' => break,
                        '\\' => string.extend(chars.next()),
                        other => string.push(other),
                    }
                }
                out.push(string);
            }
            _ => {}
        }
    }
    out
}

fn color(spec: &str) -> [u8; 4] {
    let words: Vec<&str> = spec.split_whitespace().collect();
    let mut found: HashMap<&str, String> = HashMap::new();
    let mut at = 0;
    while at < words.len() {
        let key = words[at];
        at += 1;
        if !KEYS.contains(&key) {
            continue;
        }
        let start = at;
        while at < words.len() && !KEYS.contains(&words[at]) {
            at += 1;
        }
        found
            .entry(key)
            .or_insert_with(|| words[start..at].join(" "));
    }
    PREFERRED
        .iter()
        .find_map(|key| found.get(key).and_then(|value| parse(value)))
        .unwrap_or([0, 0, 0, 255])
}

fn parse(value: &str) -> Option<[u8; 4]> {
    if value.eq_ignore_ascii_case("none") {
        return Some([0, 0, 0, 0]);
    }
    if let Some(hex) = value.strip_prefix('#') {
        let digits = hex.len() / 3;
        if !matches!(digits, 1..=4) || hex.len() != digits * 3 {
            return None;
        }
        let channel = |at: usize| {
            let value = u32::from_str_radix(hex.get(at * digits..(at + 1) * digits)?, 16).ok()?;
            let most = (1u32 << (digits * 4)) - 1;
            Some(((value * 255 + most / 2) / most) as u8)
        };
        return Some([channel(0)?, channel(1)?, channel(2)?, 255]);
    }
    named(&value.to_ascii_lowercase().replace(' ', ""))
}

fn named(name: &str) -> Option<[u8; 4]> {
    let grey = name
        .strip_prefix("gray")
        .or_else(|| name.strip_prefix("grey"));
    if let Some(level) = grey.and_then(|level| level.parse::<u32>().ok())
        && level <= 100
    {
        let value = ((level * 255 + 50) / 100) as u8;
        return Some([value, value, value, 255]);
    }
    let [red, green, blue] = match name {
        "black" => [0, 0, 0],
        "white" => [255, 255, 255],
        "red" => [255, 0, 0],
        "green" => [0, 255, 0],
        "blue" => [0, 0, 255],
        "yellow" => [255, 255, 0],
        "cyan" => [0, 255, 255],
        "magenta" => [255, 0, 255],
        "gray" | "grey" => [190, 190, 190],
        "darkgray" | "darkgrey" => [169, 169, 169],
        "lightgray" | "lightgrey" => [211, 211, 211],
        "dimgray" | "dimgrey" => [105, 105, 105],
        "orange" => [255, 165, 0],
        "brown" => [165, 42, 42],
        "navy" | "navyblue" => [0, 0, 128],
        "darkgreen" => [0, 100, 0],
        "darkred" => [139, 0, 0],
        "darkblue" => [0, 0, 139],
        "gold" => [255, 215, 0],
        "pink" => [255, 192, 203],
        "purple" => [160, 32, 240],
        "violet" => [238, 130, 238],
        "maroon" => [176, 48, 96],
        "beige" => [245, 245, 220],
        "tan" => [210, 180, 140],
        "khaki" => [240, 230, 140],
        "salmon" => [250, 128, 114],
        "wheat" => [245, 222, 179],
        "ivory" => [255, 255, 240],
        "silver" => [192, 192, 192],
        _ => return None,
    };
    Some([red, green, blue, 255])
}
