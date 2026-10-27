use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::rc::Rc;

use beui_types::{
    Color, Color32, Corners, FillRule, Image, ImageId, LineCap, LineJoin, Path, PathElement,
    PixelGrid, Pos2, Rect, Stroke, Vec2, pos2,
};

use crate::css::{self, number};
use crate::text_block::TextBlock;

pub type Draw = Rc<dyn Fn(&Painter, Rect)>;

#[derive(Default)]
pub struct Images {
    urls: HashMap<ImageId, Rc<str>>,
}

pub fn draw_svg(draw: &Draw, size: Vec2, clipped: bool, ids: &str, images: &mut Images) -> String {
    let out = Rc::new(RefCell::new(Out {
        ids: ids.to_owned(),
        images: std::mem::take(images),
        ..Out::default()
    }));
    let rect = Rect::from_min_size(Pos2::ZERO, size);
    let painter = Painter {
        out: out.clone(),
        layers: Rc::new(Vec::new()),
        clip: match clipped {
            true => rect,
            false => Rect::EVERYTHING,
        },
    };
    draw(&painter, rect);
    drop(painter);
    let mut out = std::mem::take(&mut *out.borrow_mut());
    out.close_to(0);
    let used = std::mem::take(&mut out.used);
    out.images.urls.retain(|id, _| used.contains(id));
    *images = std::mem::take(&mut out.images);
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100%\" height=\"100%\" viewBox=\"0 0 {} {}\" preserveAspectRatio=\"none\" style=\"display: block; overflow: {};\">",
        number(size.x),
        number(size.y),
        match clipped {
            true => "hidden",
            false => "visible",
        }
    );
    if !out.defs.is_empty() {
        let _ = write!(svg, "<defs>{}</defs>", out.defs);
    }
    svg.push_str(&out.body);
    svg.push_str("</svg>");
    svg
}

#[derive(Default)]
struct Out {
    ids: String,
    body: String,
    defs: String,
    open: Vec<u32>,
    next: u32,
    images: Images,
    used: Vec<ImageId>,
}

impl Out {
    fn id(&mut self, kind: char) -> String {
        self.next += 1;
        format!("{}{kind}{}", self.ids, self.next)
    }

    fn close_to(&mut self, depth: usize) {
        while self.open.len() > depth {
            self.open.pop();
            self.body.push_str("</g>");
        }
    }

    fn image_url(&mut self, image: &Image) -> Rc<str> {
        if !self.used.contains(&image.id()) {
            self.used.push(image.id());
        }
        self.images
            .urls
            .entry(image.id())
            .or_insert_with(|| png_url(image).into())
            .clone()
    }
}

#[derive(Clone)]
struct Layer {
    id: u32,
    attributes: Rc<str>,
}

#[derive(Clone)]
pub struct Painter {
    out: Rc<RefCell<Out>>,
    layers: Rc<Vec<Layer>>,
    clip: Rect,
}

impl Painter {
    pub fn pixel_grid(&self) -> PixelGrid {
        PixelGrid::new(reactive::Get::get(&crate::page::pixels_per_point()))
    }

    pub fn resolve_color(&self, color: Color) -> Color32 {
        crate::colors::resolve(color)
    }

    pub fn clip_rect(&self) -> Rect {
        self.clip
    }

    pub fn transformed(&self, offset: Vec2, scale: f32) -> Self {
        let local = |at: Pos2| pos2((at.x - offset.x) / scale, (at.y - offset.y) / scale);
        let clip = match self.clip == Rect::EVERYTHING || scale <= 0.0 {
            true => self.clip,
            false => Rect::from_min_max(local(self.clip.min), local(self.clip.max)),
        };
        self.layered(
            format!(
                "transform=\"translate({} {}) scale({})\"",
                number(offset.x),
                number(offset.y),
                number(scale)
            ),
            clip,
        )
    }

    pub fn rotated(&self, pivot: Pos2, angle: f32) -> Self {
        self.layered(
            format!(
                "transform=\"rotate({} {} {})\"",
                number(angle.to_degrees()),
                number(pivot.x),
                number(pivot.y)
            ),
            Rect::EVERYTHING,
        )
    }

    pub fn with_clip_rect(&self, clip: Rect) -> Self {
        if clip == Rect::EVERYTHING {
            return self.layered(String::new(), self.clip);
        }
        let id = {
            let mut out = self.out.borrow_mut();
            let id = out.id('c');
            let rect = rect_element(clip, Corners::ZERO, "");
            let _ = write!(out.defs, "<clipPath id=\"{id}\">{rect}</clipPath>");
            id
        };
        self.layered(
            format!("clip-path=\"url(#{id})\""),
            self.clip.intersect(clip),
        )
    }

    pub fn rect_filled(
        &self,
        rect: Rect,
        corner_radius: impl Into<Corners>,
        color: impl Into<Color>,
    ) {
        let color = color.into();
        if color.is_clear() || !rect.is_positive() {
            return;
        }
        let corners = corner_radius.into().fitted(rect);
        let style = format!("fill: {};", css::color(color));
        self.emit(&rect_element(rect, corners, &style));
    }

    pub fn rect_stroke(
        &self,
        rect: Rect,
        corner_radius: impl Into<Corners>,
        width: f32,
        color: impl Into<Color>,
    ) {
        let color = color.into();
        if color.is_clear() || width <= 0.0 || !rect.is_positive() {
            return;
        }
        let corners = corner_radius.into().fitted(rect.expand(width));
        let half = width / 2.0;
        let inset = Corners::new(
            (corners.top_left - half).max(0.0),
            (corners.top_right - half).max(0.0),
            (corners.bottom_right - half).max(0.0),
            (corners.bottom_left - half).max(0.0),
        );
        let style = format!(
            "fill: none; stroke: {}; stroke-width: {};",
            css::color(color),
            number(width)
        );
        self.emit(&rect_element(rect.shrink(half), inset, &style));
    }

    pub fn line(&self, from: Pos2, to: Pos2, width: f32, color: impl Into<Color>) {
        let color = color.into();
        if color.is_clear() || width <= 0.0 {
            return;
        }
        self.emit(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" style=\"stroke: {}; stroke-width: {};\"/>",
            number(from.x),
            number(from.y),
            number(to.x),
            number(to.y),
            css::color(color),
            number(width)
        ));
    }

    pub fn image(
        &self,
        rect: Rect,
        source: Rect,
        image: &Image,
        tint: impl Into<Color>,
        corner_radius: f32,
        smooth: bool,
    ) {
        let tint = self.resolve_color(tint.into());
        if tint.alpha() == 0 || !rect.is_positive() || image.width() == 0 || image.height() == 0 {
            return;
        }
        let size = image.size();
        let (url, clip, filter) = {
            let mut out = self.out.borrow_mut();
            let url = out.image_url(image);
            let clip = (corner_radius > 0.0).then(|| {
                let id = out.id('c');
                let shape = rect_element(rect, Corners::all(corner_radius).fitted(rect), "");
                let _ = write!(out.defs, "<clipPath id=\"{id}\">{shape}</clipPath>");
                id
            });
            let filter = (tint != Color32::WHITE).then(|| {
                let id = out.id('f');
                let [red, green, blue, alpha] = tint.to_normalized_gamma_f32();
                let _ = write!(
                    out.defs,
                    "<filter id=\"{id}\" color-interpolation-filters=\"sRGB\"><feColorMatrix type=\"matrix\" values=\"{} 0 0 0 0 0 {} 0 0 0 0 0 {} 0 0 0 0 0 {} 0\"/></filter>",
                    number(red),
                    number(green),
                    number(blue),
                    number(alpha)
                );
                id
            });
            (url, clip, filter)
        };
        let mut element = String::new();
        if let Some(clip) = &clip {
            let _ = write!(element, "<g clip-path=\"url(#{clip})\">");
        }
        let _ = write!(
            element,
            "<svg x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\" preserveAspectRatio=\"none\"><image width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{url}\"",
            number(rect.left()),
            number(rect.top()),
            number(rect.width()),
            number(rect.height()),
            number(source.left() * size.x),
            number(source.top() * size.y),
            number(source.width() * size.x),
            number(source.height() * size.y),
            number(size.x),
            number(size.y),
        );
        if let Some(filter) = &filter {
            let _ = write!(element, " filter=\"url(#{filter})\"");
        }
        if !smooth {
            element.push_str(" style=\"image-rendering: pixelated;\"");
        }
        element.push_str("/></svg>");
        if clip.is_some() {
            element.push_str("</g>");
        }
        self.emit(&element);
    }

    pub fn fill_path(&self, path: &Path, rule: FillRule, color: impl Into<Color>) {
        let color = color.into();
        if color.is_clear() || path.is_empty() {
            return;
        }
        let rule = match rule {
            FillRule::NonZero => "nonzero",
            FillRule::EvenOdd => "evenodd",
        };
        self.emit(&format!(
            "<path d=\"{}\" style=\"fill: {}; fill-rule: {rule};\"/>",
            path_data(path),
            css::color(color)
        ));
    }

    pub fn stroke_path(&self, path: &Path, stroke: &Stroke, color: impl Into<Color>) {
        let color = color.into();
        if color.is_clear() || path.is_empty() || stroke.width.is_nan() || stroke.width <= 0.0 {
            return;
        }
        let mut style = format!(
            "fill: none; stroke: {}; stroke-width: {}; stroke-linejoin: {}; stroke-linecap: {};",
            css::color(color),
            number(stroke.width),
            match stroke.join {
                LineJoin::Miter => "miter",
                LineJoin::Round => "round",
                LineJoin::Bevel => "bevel",
            },
            match stroke.cap {
                LineCap::Butt => "butt",
                LineCap::Round => "round",
                LineCap::Square => "square",
            },
        );
        if stroke.join == LineJoin::Miter {
            let _ = write!(
                style,
                " stroke-miterlimit: {};",
                number(stroke.miter_limit.max(1.0))
            );
        }
        if stroke.is_dashed() {
            let dashes: Vec<String> = stroke.dashes.iter().map(|dash| number(*dash)).collect();
            let _ = write!(
                style,
                " stroke-dasharray: {}; stroke-dashoffset: {};",
                dashes.join(" "),
                number(stroke.dash_offset)
            );
        }
        self.emit(&format!(
            "<path d=\"{}\" style=\"{style}\"/>",
            path_data(path)
        ));
    }

    pub fn draw_text(&self, origin: Pos2, block: &TextBlock, color: impl Into<Color>) {
        let color = color.into();
        if color.is_clear() {
            return;
        }
        let style = escaped(&format!(
            "font: {}; fill: {}; white-space: pre;",
            css::font(block.font()),
            css::color(color)
        ));
        for line in block.lines() {
            let shown = &block.text()[line.shown.clone()];
            if shown.trim().is_empty() {
                continue;
            }
            self.emit(&format!(
                "<text x=\"{}\" y=\"{}\" textLength=\"{}\" lengthAdjust=\"spacing\" style=\"{style}\">{}</text>",
                number(origin.x + line.left),
                number(origin.y + line.top + block.baseline()),
                number(line.width),
                escaped(shown)
            ));
        }
    }

    fn layered(&self, attributes: String, clip: Rect) -> Self {
        let mut layers = (*self.layers).clone();
        if !attributes.is_empty() {
            let id = {
                let mut out = self.out.borrow_mut();
                out.next += 1;
                out.next
            };
            layers.push(Layer {
                id,
                attributes: attributes.into(),
            });
        }
        Self {
            out: self.out.clone(),
            layers: Rc::new(layers),
            clip,
        }
    }

    fn emit(&self, element: &str) {
        let mut out = self.out.borrow_mut();
        let common = out
            .open
            .iter()
            .zip(self.layers.iter())
            .take_while(|(open, layer)| **open == layer.id)
            .count();
        out.close_to(common);
        for layer in &self.layers[common..] {
            let _ = write!(out.body, "<g {}>", layer.attributes);
            out.open.push(layer.id);
        }
        out.body.push_str(element);
    }
}

fn rect_element(rect: Rect, corners: Corners, style: &str) -> String {
    let style = match style.is_empty() {
        true => String::new(),
        false => format!(" style=\"{style}\""),
    };
    let [top_left, top_right, bottom_right, bottom_left] = corners.to_array();
    if top_left == top_right && top_left == bottom_right && top_left == bottom_left {
        let radius = match top_left > 0.0 {
            true => format!(" rx=\"{}\"", number(top_left)),
            false => String::new(),
        };
        return format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{radius}{style}/>",
            number(rect.left()),
            number(rect.top()),
            number(rect.width()),
            number(rect.height())
        );
    }
    let (left, top, right, bottom) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    let arc = |radius: f32, x: f32, y: f32| match radius > 0.0 {
        true => format!(
            " A {} {} 0 0 1 {} {}",
            number(radius),
            number(radius),
            number(x),
            number(y)
        ),
        false => String::new(),
    };
    format!(
        "<path d=\"M {} {} H {}{} V {}{} H {}{} V {}{} Z\"{style}/>",
        number(left + top_left),
        number(top),
        number(right - top_right),
        arc(top_right, right, top + top_right),
        number(bottom - bottom_right),
        arc(bottom_right, right - bottom_right, bottom),
        number(left + bottom_left),
        arc(bottom_left, left, bottom - bottom_left),
        number(top + top_left),
        arc(top_left, left + top_left, top),
    )
}

fn path_data(path: &Path) -> String {
    let mut data = String::new();
    let mut point = |command: &str, points: &[Pos2]| {
        data.push_str(command);
        for at in points {
            let _ = write!(data, " {} {}", number(at.x), number(at.y));
        }
    };
    for element in path.elements() {
        match *element {
            PathElement::MoveTo(to) => point(" M", &[to]),
            PathElement::LineTo(to) => point(" L", &[to]),
            PathElement::QuadTo(control, to) => point(" Q", &[control, to]),
            PathElement::CubicTo(first, second, to) => point(" C", &[first, second, to]),
            PathElement::Close => point(" Z", &[]),
        }
    }
    data.trim_start().to_owned()
}

fn escaped(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(c),
        }
    }
    output
}

fn png_url(image: &Image) -> String {
    use base64::Engine as _;

    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, image.width(), image.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let written = encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(image.pixels()));
    if written.is_err() {
        return String::new();
    }
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}
