use beui::icons::{ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_EXTERNAL_LINK, ICON_X};
use beui::reactive::*;
use beui::styled::*;
use beui::{CursorIcon, Image, ImageFit, Key, KeyPress, NodeId, TextAlign, Vec2};

use crate::model::{Loaded, Model};
use beui::{Align, Direction};

const PADDING: Space = Space::Xl;
const HINT_HEIGHT: f32 = 18.0;

#[component]
pub(crate) fn ImageViewer(model: Model) -> NodeId {
    let open = create_memo(move || model.viewer.with(Option::is_some));
    let current = create_memo(move || {
        model
            .viewer
            .get()
            .and_then(|viewer| viewer.images.get(viewer.index).cloned())
    });
    let picture = create_memo(move || {
        let url = current.get()?;
        match model.image(&url).get() {
            Loaded::Ready(image) => Some(image),
            _ => None,
        }
    });
    let position = create_memo(move || match model.viewer.get() {
        Some(viewer) => format!(
            "Image {} of {} · the arrow keys move between them",
            viewer.index + 1,
            viewer.images.len()
        ),
        None => String::new(),
    });
    let first = create_memo(move || {
        model
            .viewer
            .with(|viewer| viewer.as_ref().is_none_or(|viewer| viewer.index == 0))
    });
    let last = create_memo(move || {
        model.viewer.with(|viewer| {
            viewer
                .as_ref()
                .is_none_or(|viewer| viewer.index + 1 >= viewer.images.len())
        })
    });
    let close = move || model.close_image();
    let previous = move || model.step_image(-1);
    let next = move || model.step_image(1);
    let browse = move || {
        if let Some(url) = current.get_untracked() {
            model.open(&url);
        }
    };
    let keys = move |press: KeyPress| {
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
    };
    view! {
        <Fullscreen open={open} on_dismiss={close}>
            <Frame fill=ColorRole::Background padding=PADDING>
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
                    <ListItem grow=1.0>
                        <Interactive focusable=true focused={open} on_key={keys}>
                            <ImageStage picture />
                        </Interactive>
                    </ListItem>
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
    on_previous: Callback<()>,
    on_next: Callback<()>,
    on_browse: Callback<()>,
    on_close: Callback<()>,
) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
            <ListItem grow=1.0>
                <Text style=TextStyle::CAPTION>{position}</Text>
            </ListItem>
            <Button
                icon_only=true
                glyph=ICON_CHEVRON_LEFT
                label="Previous image"
                disabled={first}
                on_click={on_previous}
            />
            <Button
                icon_only=true
                glyph=ICON_CHEVRON_RIGHT
                label="Next image"
                disabled={last}
                on_click={on_next}
            />
            <Button
                icon_only=true
                glyph=ICON_EXTERNAL_LINK
                label="Open in the browser"
                on_click={on_browse}
            />
            <Button
                icon_only=true
                @test_id={"viewer.close"}
                glyph=ICON_X
                label="Close"
                on_click={on_close}
            />
        </List>
    }
}

#[component]
fn ImageStage(picture: Memo<Option<Image>>) -> NodeId {
    let size_held = create_signal(Vec2::ZERO);
    let size = size_held.read_only();
    let natural = create_memo(move || picture.get().map_or(Vec2::ZERO, |image| image.size()));
    let shrunk = create_memo(move || {
        let natural = natural.get();
        let room = size.get() - Vec2::new(0.0, HINT_HEIGHT);
        natural.x > room.x || natural.y > room.y
    });
    let actual = create_signal(false);
    let zoomed = create_memo(move || actual.get() && shrunk.get());
    let fitted = create_memo(move || !zoomed.get());
    let cursor = create_memo(move || match shrunk.get() {
        true => CursorIcon::PointingHand,
        false => CursorIcon::Default,
    });
    let hint = create_memo(move || {
        match (zoomed.get(), shrunk.get()) {
            (true, _) => "100% · click to fit it to the window",
            (false, true) => "Scaled down to fit · click for 100%",
            (false, false) => "100%",
        }
        .to_owned()
    });
    let width = create_memo(move || Size::Custom(natural.get().x));
    let height = create_memo(move || Size::Custom(natural.get().y));
    let zoom_in = move || {
        if shrunk.get_untracked() {
            actual.set(true);
        }
    };
    let zoom_out = move || actual.set(false);
    let built = view! {
        <List spacing=Space::Md>
            <Show condition={fitted}>
                <ListItem grow=1.0>
                    <Interactive cursor={cursor} on_click={zoom_in}>
                        <Picture image={picture} fit=ImageFit::ScaleDown smooth={shrunk} />
                    </Interactive>
                </ListItem>
            </Show>
            <Show condition={zoomed}>
                <ListItem grow=1.0>
                    <Scroll>
                        <Scroll direction=Direction::Horizontal>
                            <Interactive cursor=CursorIcon::PointingHand on_click={zoom_out}>
                                <Frame width=width height=height>
                                    <Picture image={picture} fit=ImageFit::Fill smooth=false />
                                </Frame>
                            </Interactive>
                        </Scroll>
                    </Scroll>
                </ListItem>
            </Show>
            <ListItem length=HINT_HEIGHT>
                <Text style=TextStyle::CAPTION align=TextAlign::Center>{hint}</Text>
            </ListItem>
        </List>
    };
    let size_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = size_measured.get() {
            size_held.set(measured.size);
        }
    });
    built
}
