use std::rc::Rc;

use beui_view::reactive::Painter;
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Color32;
use beui_view::types::{Image, ImageFit, Thumbhash};
use beui_view::types::{Pos2, Rect, Vec2, pos2};

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

fn visible_part(placed: Rect, uv: Rect, rect: Rect) -> (Rect, Rect) {
    let visible = placed.intersect(rect);
    if visible == placed || visible.width() <= 0.0 || visible.height() <= 0.0 {
        return (placed, uv);
    }
    let at = |point: Pos2| {
        pos2(
            uv.left() + (point.x - placed.left()) / placed.width() * uv.width(),
            uv.top() + (point.y - placed.top()) / placed.height() * uv.height(),
        )
    };
    (
        visible,
        Rect::from_min_max(at(visible.min), at(visible.max)),
    )
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
    let size = create_memo(move || {
        let source = source.get();
        shown.with(|shown| shown.as_ref().map(|shown| cropped(shown.size, source)))
    });
    let draw = Prop::dynamic(move || -> Draw {
        let shown = shown.get();
        let source = source.get();
        let (fit, tint, radius) = (fit.get(), tint.get(), radius.get());
        Rc::new(move |painter: &Painter, rect: Rect| {
            let Some(shown) = shown.as_ref() else {
                return;
            };
            let placed = fit.place(rect, cropped(shown.size, source));
            let uv = source.unwrap_or(Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)));
            let (placed, uv) = visible_part(placed, uv, rect);
            painter.image(placed, uv, &shown.image, tint, radius, shown.smooth);
        })
    });
    view! {
        <Drawing draw size={size} />
    }
}
