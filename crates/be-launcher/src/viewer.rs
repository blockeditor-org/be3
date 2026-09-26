use beui::icons::{ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_CLOSE, ICON_OPEN_IN_NEW};
use beui::reactive::{
    Align, Direction, Focusable, Frame, ItemSize, List, Picture, Spacer, clone, component,
    create_memo, view,
};
use beui::styled::{Caption, Fullscreen, IconButton, use_theme};
use beui::{ImageFit, Key, KeyPress, NodeId};

use crate::model::{Loaded, Model};

const PADDING: f32 = 16.0;

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
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Caption content={position} />
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                        <IconButton
                            glyph=ICON_CHEVRON_LEFT
                            label="Previous image"
                            disabled={first}
                            on_click={previous}
                        />
                        <IconButton
                            glyph=ICON_CHEVRON_RIGHT
                            label="Next image"
                            disabled={last}
                            on_click={next}
                        />
                        <IconButton
                            glyph=ICON_OPEN_IN_NEW
                            label="Open in the browser"
                            on_click={browse}
                        />
                        <IconButton glyph=ICON_CLOSE label="Close" on_click={close} />
                    </List>
                    <Focusable @sizing=ItemSize::Percent(100.0) focused={open} on_key={keys}>
                        <Picture image={picture} fit=ImageFit::Contain smooth=false />
                    </Focusable>
                </List>
            </Frame>
        </Fullscreen>
    }
}
