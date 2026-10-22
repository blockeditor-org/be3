use beui_core::image::Image;

pub trait Dom {
    type Element: Clone;

    fn root(&self) -> Self::Element;

    fn div(&mut self) -> Option<Self::Element>;

    fn set_class(&mut self, element: &Self::Element, class: &str);

    fn set_css(&mut self, element: &Self::Element, css: &str);

    fn set_style(&mut self, element: &Self::Element, property: &str, value: &str);

    fn set_text(&mut self, element: &Self::Element, text: Option<&str>);

    fn insert_before(
        &mut self,
        parent: &Self::Element,
        child: &Self::Element,
        next: Option<&Self::Element>,
    );

    fn remove_from(&mut self, parent: &Self::Element, child: &Self::Element);

    fn add_stylesheet(&mut self, css: &str);

    fn image_url(&mut self, image: &Image) -> Option<String>;

    fn device_pixel_ratio(&self) -> f32;
}
