use beui::icons::{ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_CLOSE, ICON_OPEN_IN_NEW};
use beui::reactive::{
    Align, ClickCallback, Direction, Frame, Interactive, ItemSize, List, Memo, Prop, Show, clone,
    component, component_size, create_memo, create_signal, view,
};
use beui::styled::{Caption, Fullscreen, IconButton, Scroll, use_theme};
use beui::unstyled::Picture;
use beui::{CursorIcon, Image, ImageFit, Key, KeyPress, NodeId, TextAlign, Vec2};

use crate::model::{Loaded, Model};

const PADDING: f32 = 16.0;
const HINT_HEIGHT: f32 = 18.0;

#[component]
pub(crate) fn ImageViewer(model: Model) -> NodeId {
    let open = create_memo(clone!(model -> move || model.viewer.with(Option::is_some)));
    let current = create_memo(clone!(model -> move || {
        model
            .viewer
            .get()
            .and_then(|viewer| viewer.images.get(viewer.index).cloned())
    }));
    let picture = create_memo(clone!(model current -> move || {
        let url = current.get()?;
        match model.image(&url).get() {
            Loaded::Ready(image) => Some(image),
            _ => None,
        }
    }));
    let position = create_memo(clone!(model -> move || match model.viewer.get() {
        Some(viewer) => format!(
            "Image {} of {} · the arrow keys move between them",
            viewer.index + 1,
            viewer.images.len()
        ),
        None => String::new(),
    }));
    let first = create_memo(clone!(model -> move || {
        model.viewer.with(|viewer| viewer.as_ref().is_none_or(|viewer| viewer.index == 0))
    }));
    let last = create_memo(clone!(model -> move || {
        model.viewer.with(|viewer| {
            viewer
                .as_ref()
                .is_none_or(|viewer| viewer.index + 1 >= viewer.images.len())
        })
    }));
    let theme = use_theme();
    let close = clone!(model -> move || model.close_image());
    let dismiss = close.clone();
    let previous = clone!(model -> move || model.step_image(-1));
    let next = clone!(model -> move || model.step_image(1));
    let browse = clone!(model current -> move || {
        if let Some(url) = current.get_untracked() {
            model.open(&url);
        }
    });
    let keys = clone!(model -> move |press: KeyPress| {
        if !press.pressed {
            return false;
        }
        match press.key {
            Key::ArrowLeft | Key::ArrowUp => model.step_image(-1),
            Key::ArrowRight | Key::ArrowDown => model.step_image(1),
            Key::Home => model.step_image(isize::MIN / 2),
            Key::End => model.step_image(isize::MAX / 2),
            _ => return false,
        }
        true
    });
    view! {
        <Fullscreen open={open.clone()} on_dismiss={dismiss}>
            <Frame
                color={theme.background.clone()}
                padding_horizontal=PADDING
                padding_vertical=PADDING
            >
                <List spacing=PADDING>
                    <ViewerBar
                        position
                        first
                        last
                        on_previous={previous}
                        on_next={next}
                        on_browse={browse}
                        on_close={close}
                    />
                    <Interactive
                        focusable=true
                        @sizing=ItemSize::Percent(100.0)
                        focused={open}
                        on_key={keys}
                    >
                        <ImageStage picture />
                    </Interactive>
                </List>
            </Frame>
        </Fullscreen>
    }
}

#[component]
pub(crate) fn ViewerBar(
    position: Prop<String>,
    first: Prop<bool>,
    last: Prop<bool>,
    on_previous: ClickCallback,
    on_next: ClickCallback,
    on_browse: ClickCallback,
    on_close: ClickCallback,
) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
            <Caption @sizing=ItemSize::Percent(100.0) content={position} wrap=true />
            <IconButton
                glyph=ICON_CHEVRON_LEFT
                label="Previous image"
                disabled={first}
                on_click={move || on_previous.call()}
            />
            <IconButton
                glyph=ICON_CHEVRON_RIGHT
                label="Next image"
                disabled={last}
                on_click={move || on_next.call()}
            />
            <IconButton
                glyph=ICON_OPEN_IN_NEW
                label="Open in the browser"
                on_click={move || on_browse.call()}
            />
            <IconButton
                @test_id={"viewer.close"}
                glyph=ICON_CLOSE
                label="Close"
                on_click={move || on_close.call()}
            />
        </List>
    }
}

#[component]
fn ImageStage(picture: Memo<Option<Image>>) -> NodeId {
    let size = component_size();
    let natural = create_memo(clone!(picture -> move || {
        picture.get().map_or(Vec2::ZERO, |image| image.size())
    }));
    let shrunk = create_memo(clone!(natural -> move || {
        let natural = natural.get();
        let room = size.get() - Vec2::new(0.0, HINT_HEIGHT);
        natural.x > room.x || natural.y > room.y
    }));
    let (actual, set_actual) = create_signal(false);
    let zoomed = create_memo(clone!(actual shrunk -> move || actual.get() && shrunk.get()));
    let fitted = create_memo(clone!(zoomed -> move || !zoomed.get()));
    let cursor = create_memo(clone!(shrunk -> move || match shrunk.get() {
        true => CursorIcon::PointingHand,
        false => CursorIcon::Default,
    }));
    let hint = create_memo(clone!(zoomed shrunk -> move || {
        match (zoomed.get(), shrunk.get()) {
            (true, _) => "100% · click to fit it to the window",
            (false, true) => "Scaled down to fit · click for 100%",
            (false, false) => "100%",
        }
        .to_owned()
    }));
    let width = create_memo(clone!(natural -> move || Some(natural.get().x)));
    let height = create_memo(clone!(natural -> move || Some(natural.get().y)));
    let zoom_in = clone!(set_actual shrunk -> move || {
        if shrunk.get_untracked() {
            set_actual.set(true);
        }
    });
    let zoom_out = move || set_actual.set(false);
    let whole = picture.clone();
    view! {
        <List spacing=8.0>
            <Show condition={fitted}>
                <Interactive @sizing=ItemSize::Percent(100.0) cursor={cursor} on_click={zoom_in}>
                    <Picture image={whole} fit=ImageFit::ScaleDown smooth={shrunk.clone()} />
                </Interactive>
            </Show>
            <Show condition={zoomed}>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <Scroll direction=Direction::Horizontal>
                        <Interactive cursor=CursorIcon::PointingHand on_click={zoom_out}>
                            <Frame width={width} height={height}>
                                <Picture image={picture} fit=ImageFit::Fill smooth=false />
                            </Frame>
                        </Interactive>
                    </Scroll>
                </Scroll>
            </Show>
            <Caption @sizing=ItemSize::Fixed(HINT_HEIGHT) content={hint} align=TextAlign::Center />
        </List>
    }
}
