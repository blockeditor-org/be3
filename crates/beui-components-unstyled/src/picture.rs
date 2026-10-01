use std::rc::Rc;

use beui_core::color::Color32;
use beui_core::geometry::{Rect, Vec2, pos2};
use beui_core::image::{Image, ImageFit, Thumbhash};
use beui_core::node::NodeId;
use beui_core::painter::Painter;
use beui_macros::{component, view};
use beui_view::reactive::{Draw, Drawing, Prop, clone, create_memo};

#[derive(Clone, PartialEq)]
struct Shown {
    image: Image,
    size: Vec2,
    smooth: bool,
}

fn cropped(size: Vec2, source: Option<Rect>) -> Vec2 {
    match source {
        Some(source) => Vec2::new(size.x * source.width(), size.y * source.height()),
        None => size,
    }
}

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
    let placeholder = create_memo(move || {
        thumbhash
            .get()
            .and_then(|thumbhash| Some((thumbhash.decode()?, thumbhash.size())))
    });
    let shown = create_memo(move || match (image.get(), placeholder.get()) {
        (Some(image), _) => Some(Shown {
            size: image.size(),
            image,
            smooth: smooth.get(),
        }),
        (None, Some((image, size))) => Some(Shown {
            image,
            size,
            smooth: true,
        }),
        (None, None) => None,
    });
    let size = create_memo(clone!(shown source -> move || {
        let source = source.get();
        shown.with(|shown| shown.as_ref().map(|shown| cropped(shown.size, source)))
    }));
    let draw = Prop::Dynamic(Rc::new(move || -> Draw {
        let shown = shown.get();
        let source = source.get();
        let (fit, tint, radius) = (fit.get(), tint.get(), radius.get());
        Rc::new(move |painter: &Painter, rect: Rect| {
            let Some(shown) = shown.as_ref() else {
                return;
            };
            painter.image(
                fit.place(rect, cropped(shown.size, source)),
                source.unwrap_or(Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))),
                &shown.image,
                tint,
                radius,
                shown.smooth,
            );
        })
    }));
    view! {
        <Drawing draw size={size} />
    }
}
