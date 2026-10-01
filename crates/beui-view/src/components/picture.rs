use beui_macros::component;

use crate::reactive::{Prop, create_effect, with_document};
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::geometry::Rect;
use beui_core::image::{Image, ImageFit, Thumbhash};
use beui_core::node::NodeId;

#[component]
pub fn Picture(
    image: Prop<Option<Image>>,
    #[prop(default = None)] thumbhash: Prop<Option<Thumbhash>>,
    #[prop(default = None)] source: Prop<Option<Rect>>,
    #[prop(default = ImageFit::Contain)] fit: Prop<ImageFit>,
    #[prop(default = Color32::WHITE)] tint: Prop<Color32>,
    #[prop(default = 0.0)] radius: Prop<f32>,
    #[prop(default = true)] smooth: Prop<bool>,
) -> NodeId {
    let picture = with_document(Document::create_picture);
    create_effect(move || {
        let image = image.get();
        with_document(|document| document.set_picture_image(picture, image));
    });
    create_effect(move || {
        let thumbhash = thumbhash.get();
        with_document(|document| document.set_picture_thumbhash(picture, thumbhash));
    });
    create_effect(move || {
        let source = source.get();
        with_document(|document| document.set_picture_source(picture, source));
    });
    create_effect(move || {
        let fit = fit.get();
        with_document(|document| document.set_picture_fit(picture, fit));
    });
    create_effect(move || {
        let tint = tint.get();
        with_document(|document| document.set_picture_tint(picture, tint));
    });
    create_effect(move || {
        let radius = radius.get();
        with_document(|document| document.set_picture_radius(picture, radius));
    });
    create_effect(move || {
        let smooth = smooth.get();
        with_document(|document| document.set_picture_smooth(picture, smooth));
    });
    picture.id()
}
