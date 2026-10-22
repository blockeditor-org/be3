use std::collections::BTreeMap;
use std::fmt::Write as _;

use beui_core::image::Image;

use crate::dom::Dom;

#[derive(Default)]
struct Element {
    class: String,
    css: String,
    properties: Vec<(String, String)>,
    text: Option<String>,
    parent: Option<usize>,
    children: Vec<usize>,
}

pub struct MemoryDom {
    elements: Vec<Element>,
    stylesheets: Vec<String>,
    images: BTreeMap<String, Image>,
}

impl MemoryDom {
    pub fn new(width: f32, height: f32) -> Self {
        let mut dom = Self {
            elements: vec![Element::default()],
            stylesheets: Vec::new(),
            images: BTreeMap::new(),
        };
        dom.set_class(&0, "beui-root");
        dom.set_css(&0, &format!("width:{width}px;height:{height}px;"));
        dom
    }

    pub fn html(&self) -> String {
        let mut html = String::from("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><style>");
        html.push_str("body{margin:0}");
        for sheet in &self.stylesheets {
            html.push_str(sheet);
        }
        html.push_str("</style></head><body>");
        self.write(0, &mut html);
        html.push_str("</body></html>");
        html
    }

    pub fn images(&self) -> &BTreeMap<String, Image> {
        &self.images
    }

    fn write(&self, index: usize, html: &mut String) {
        let element = &self.elements[index];
        html.push_str("<div");
        if !element.class.is_empty() {
            let _ = write!(html, " class=\"{}\"", escape(&element.class));
        }
        let mut style = element.css.clone();
        for (property, value) in &element.properties {
            let _ = write!(style, "{property}:{value};");
        }
        if !style.is_empty() {
            let _ = write!(html, " style=\"{}\"", escape(&style));
        }
        html.push('>');
        if let Some(text) = &element.text {
            html.push_str(&escape(text));
        }
        for child in &element.children {
            self.write(*child, html);
        }
        html.push_str("</div>");
    }

    fn detach(&mut self, child: usize) {
        if let Some(parent) = self.elements[child].parent.take() {
            self.elements[parent].children.retain(|held| *held != child);
        }
    }
}

impl Dom for MemoryDom {
    type Element = usize;

    fn root(&self) -> usize {
        0
    }

    fn div(&mut self) -> Option<usize> {
        self.elements.push(Element::default());
        Some(self.elements.len() - 1)
    }

    fn set_class(&mut self, element: &usize, class: &str) {
        class.clone_into(&mut self.elements[*element].class);
    }

    fn set_css(&mut self, element: &usize, css: &str) {
        let element = &mut self.elements[*element];
        css.clone_into(&mut element.css);
        element.properties.clear();
    }

    fn set_style(&mut self, element: &usize, property: &str, value: &str) {
        let properties = &mut self.elements[*element].properties;
        properties.retain(|(held, _)| held != property);
        if !value.is_empty() {
            properties.push((property.to_owned(), value.to_owned()));
        }
    }

    fn set_text(&mut self, element: &usize, text: Option<&str>) {
        for child in std::mem::take(&mut self.elements[*element].children) {
            self.elements[child].parent = None;
        }
        self.elements[*element].text = text.filter(|text| !text.is_empty()).map(str::to_owned);
    }

    fn insert_before(&mut self, parent: &usize, child: &usize, next: Option<&usize>) {
        self.detach(*child);
        let children = &self.elements[*parent].children;
        let at = next
            .and_then(|next| children.iter().position(|held| held == next))
            .unwrap_or(children.len());
        self.elements[*parent].children.insert(at, *child);
        self.elements[*child].parent = Some(*parent);
    }

    fn remove_from(&mut self, parent: &usize, child: &usize) {
        if self.elements[*child].parent == Some(*parent) {
            self.detach(*child);
        }
    }

    fn add_stylesheet(&mut self, css: &str) {
        self.stylesheets.push(css.to_owned());
    }

    fn image_url(&mut self, image: &Image) -> Option<String> {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let size = [image.width().to_le_bytes(), image.height().to_le_bytes()];
        for byte in size.iter().flatten().chain(image.pixels()) {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
        }
        let url = format!("/images/{hash:016x}.png");
        self.images.insert(url.clone(), image.clone());
        Some(url)
    }

    fn device_pixel_ratio(&self) -> f32 {
        1.0
    }
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            character => escaped.push(character),
        }
    }
    escaped
}
