use beui_types::{
    Align, Color, Color32, ColorSource, Corners, Direction, Extent, FontFamily, FontId, Justify,
    Outline, Sides, Track,
};

pub fn number(value: f32) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    if rounded == rounded.trunc() {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

pub fn px(value: f32) -> String {
    format!("{}px", number(value))
}

pub fn color(color: Color) -> String {
    if let Some(literal) = color.as_literal() {
        return literal_color(literal);
    }
    let source = |source: ColorSource| match source {
        ColorSource::Literal(color) => literal_color(color),
        ColorSource::Var(var) => format!("var(--{})", var.css_name()),
    };
    let mut out = source(color.source());
    if let Some((other, share)) = color.mixed() {
        out = format!(
            "color-mix(in srgb, {out}, {} {}%)",
            source(other),
            number(share * 100.0)
        );
    }
    match color.alpha_factor() {
        1.0 => out,
        alpha => format!(
            "color-mix(in srgb, {out} {}%, transparent)",
            number(alpha * 100.0)
        ),
    }
}

pub fn literal_color(color: Color32) -> String {
    let [red, green, blue, alpha] = color.to_array();
    match alpha {
        255 => format!("#{red:02x}{green:02x}{blue:02x}"),
        alpha => format!(
            "rgba({red}, {green}, {blue}, {})",
            number(f32::from(alpha) / 255.0)
        ),
    }
}

pub fn sides(sides: Sides) -> String {
    format!(
        "{} {} {} {}",
        px(sides.top),
        px(sides.right),
        px(sides.bottom),
        px(sides.left)
    )
}

pub fn corners(corners: Corners) -> String {
    format!(
        "{} {} {} {}",
        px(corners.top_left),
        px(corners.top_right),
        px(corners.bottom_right),
        px(corners.bottom_left)
    )
}

pub fn outline(outline: Outline) -> (String, String) {
    (
        format!("{} solid {}", px(outline.width), color(outline.color)),
        px(outline.offset - outline.width),
    )
}

pub fn align(align: Align) -> &'static str {
    match align {
        Align::Start => "start",
        Align::Center => "center",
        Align::End => "end",
        Align::Stretch => "stretch",
        Align::Baseline => "baseline",
    }
}

pub fn justify(justify: Justify) -> &'static str {
    match justify {
        Justify::Start => "flex-start",
        Justify::Center => "center",
        Justify::End => "flex-end",
        Justify::SpaceBetween => "space-between",
        Justify::SpaceAround => "space-around",
        Justify::SpaceEvenly => "space-evenly",
    }
}

pub fn flex_direction(direction: Direction) -> &'static str {
    match direction {
        Direction::Horizontal => "row",
        Direction::Vertical => "column",
    }
}

pub struct Length {
    pub size: Option<String>,
    pub min: Option<String>,
    pub max: Option<String>,
}

pub fn extent(extent: Extent) -> Length {
    let size = match (extent.exact, extent.fraction, extent.max) {
        (Some(exact), _, _) => Some(px(exact)),
        (None, Some(fraction), _) => Some(format!("{}%", number(fraction * 100.0))),
        (None, None, Some(_)) => Some("100%".to_owned()),
        (None, None, None) => None,
    };
    Length {
        size,
        min: extent.min.map(px),
        max: extent.max.map(px),
    }
}

pub fn track(track: Track) -> String {
    match track {
        Track::Fixed(length) => px(length),
        Track::Intrinsic => "max-content".to_owned(),
        Track::Fraction(share) => format!("minmax(0, {}fr)", number(share)),
    }
}

pub fn tracks(tracks: &[Track]) -> String {
    tracks
        .iter()
        .map(|column| track(*column))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn family(family: FontFamily) -> &'static str {
    match family {
        FontFamily::Proportional => crate::fonts::PROPORTIONAL,
        FontFamily::Monospace => crate::fonts::MONOSPACE,
        FontFamily::Icons => crate::fonts::ICONS,
    }
}

pub fn font(font: FontId) -> String {
    format!(
        "{}{}{} {}",
        if font.italic { "italic " } else { "" },
        if font.bold { "bold " } else { "" },
        px(font.size),
        family(font.family)
    )
}

pub fn fade(fade: Sides) -> Option<String> {
    let axis = |to: &str, start: f32, end: f32| {
        (start > 0.0 || end > 0.0).then(|| {
            format!(
                "linear-gradient(to {to}, transparent 0px, #000 {}, #000 calc(100% - {}), transparent 100%)",
                px(start),
                px(end)
            )
        })
    };
    let layers: Vec<String> = [
        axis("right", fade.left, fade.right),
        axis("bottom", fade.top, fade.bottom),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!layers.is_empty()).then(|| layers.join(", "))
}
