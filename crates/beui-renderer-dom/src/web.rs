use std::error::Error;

use wasm_bindgen::JsCast;

use beui_core::image::Image;

use crate::dom::Dom;

pub struct WebDom {
    document: web_sys::Document,
    root: web_sys::HtmlElement,
}

impl WebDom {
    pub fn new(root: web_sys::HtmlElement) -> Result<Self, Box<dyn Error>> {
        let document = root
            .owner_document()
            .ok_or("the element is not in a document")?;
        document.head().ok_or("the page has no head")?;
        root.class_list()
            .add_1("beui-root")
            .map_err(|_| "could not style the root element")?;
        Ok(Self { document, root })
    }
}

impl Dom for WebDom {
    type Element = web_sys::HtmlElement;

    fn root(&self) -> Self::Element {
        self.root.clone()
    }

    fn div(&mut self) -> Option<Self::Element> {
        self.document
            .create_element("div")
            .ok()?
            .dyn_into::<web_sys::HtmlElement>()
            .ok()
    }

    fn set_class(&mut self, element: &Self::Element, class: &str) {
        element.set_class_name(class);
    }

    fn set_css(&mut self, element: &Self::Element, css: &str) {
        element.style().set_css_text(css);
    }

    fn set_style(&mut self, element: &Self::Element, property: &str, value: &str) {
        let _ = element.style().set_property(property, value);
    }

    fn set_text(&mut self, element: &Self::Element, text: Option<&str>) {
        element.set_text_content(text);
    }

    fn insert_before(
        &mut self,
        parent: &Self::Element,
        child: &Self::Element,
        next: Option<&Self::Element>,
    ) {
        let next: Option<&web_sys::Node> = next.map(AsRef::as_ref);
        let _ = parent.insert_before(child, next);
    }

    fn remove_from(&mut self, parent: &Self::Element, child: &Self::Element) {
        let owner: &web_sys::Node = parent.as_ref();
        if child
            .parent_node()
            .is_some_and(|node| node.is_same_node(Some(owner)))
        {
            child.remove();
        }
    }

    fn add_stylesheet(&mut self, css: &str) {
        let Ok(style) = self.document.create_element("style") else {
            return;
        };
        style.set_text_content(Some(css));
        if let Some(head) = self.document.head() {
            let _ = head.append_child(&style);
        }
    }

    fn image_url(&mut self, image: &Image) -> Option<String> {
        let canvas = self
            .document
            .create_element("canvas")
            .ok()?
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .ok()?;
        canvas.set_width(image.width());
        canvas.set_height(image.height());
        let context = canvas
            .get_context("2d")
            .ok()??
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .ok()?;
        let pixels = js_sys::Uint8ClampedArray::new_with_length(image.pixels().len() as u32);
        pixels.copy_from(image.pixels());
        let data = web_sys::ImageData::new_with_js_u8_clamped_array_and_sh(
            &pixels,
            image.width(),
            image.height(),
        )
        .ok()?;
        context.put_image_data(&data, 0.0, 0.0).ok()?;
        canvas.to_data_url().ok()
    }

    fn device_pixel_ratio(&self) -> f32 {
        web_sys::window().map_or(1.0, |window| window.device_pixel_ratio() as f32)
    }
}
