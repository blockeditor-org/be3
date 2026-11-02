use std::rc::Rc;

use beui::native::Document;
use beui::reactive::*;
use beui::styled::*;
use beui::{Key, KeyAt, KeyEvent, KeyPhase, KeyPress, NodeId, Rect, native::Context};

mod gallery;
mod images;
mod review;
mod status;
mod viewer;

pub use images::{ImageRequest, ImageState, Images};

use be_tool_paint_review_manifest::Manifest;
use beui::{Align, Direction, ItemSize};
use beui_host::native::build;
use gallery::Gallery;
use review::Review;
use viewer::Viewer;

const MESSAGE_SPACING: Space = Space::Md;
const MESSAGE_PADDING: Space = Space::Xl3;
const HINT_PADDING: Padding = Padding::symmetric(Space::Xl, Space::Sm);

#[derive(Clone, Debug, PartialEq)]
pub enum Fetch<T> {
    Loading,
    Ready(T),
    Failed(String),
}

pub struct PaintReviewProgram {
    pub document: Document,
    manifest: OwnedSignal<Fetch<Rc<Manifest>>>,
    images: OwnedStored<images::ImageTable>,
}

impl PaintReviewProgram {
    pub fn new(painting: Option<String>, request: impl Fn(ImageRequest) + 'static) -> Self {
        let manifest = OwnedSignal::new(Fetch::Loading);
        let images = Images::owned(request);
        let shown = manifest.read_only();
        let table = Images::new(images.handle());
        Self {
            document: build(move || {
                view! {
                    <Root title="Paintings" clear_color=ColorRole::Background>
                        <PaintReview manifest={shown} images={table} painting={painting.clone()} />
                    </Root>
                }
            }),
            manifest,
            images,
        }
    }

    pub fn manifest(&self) -> Signal<Fetch<Rc<Manifest>>> {
        self.manifest.handle()
    }

    pub fn images(&self) -> Images {
        Images::new(self.images.handle())
    }
}

impl beui::native::Program for PaintReviewProgram {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}

#[component]
fn PaintReview(
    manifest: ReadSignal<Fetch<Rc<Manifest>>>,
    images: Images,
    painting: Prop<Option<String>>,
) -> NodeId {
    let review = Review::new(manifest, images, painting.get_untracked());
    on_key(
        KeyAt::Node(component_node()),
        KeyPhase::Capture,
        move |event: KeyEvent| event.press().is_some_and(|press| shortcut(review, press)),
    );
    let loading = create_memo(move || manifest.with(|manifest| matches!(manifest, Fetch::Loading)));
    let failure = create_memo(move || match manifest.get() {
        Fetch::Failed(error) => Some(error),
        _ => None,
    });
    let failed = create_memo(move || failure.get().is_some());
    let reason = create_memo(move || failure.get().unwrap_or_default());
    let ready = create_memo(move || manifest.with(|manifest| matches!(manifest, Fetch::Ready(_))));
    let opened = create_memo(move || ready.get() && review.open.get().is_some());
    let browsing = create_memo(move || ready.get() && review.open.get().is_none());
    view! {
        <Frame fill=ColorRole::Background>
            <List spacing=Space::None>
                <Show condition={loading}>
                    <Frame @sizing=ItemSize::Percent(100.0) align=Align::Center>
                        <List
                            align=Align::Center
                            spacing=MESSAGE_SPACING
                            @test_id={"paint_review.loading"}
                        >
                            <Progress width=INLINE_PROGRESS_WIDTH label="Loading the paintings" />
                            <Text style=TextStyle::CAPTION>"Loading the paintings"</Text>
                        </List>
                    </Frame>
                </Show>
                <Show condition={failed}>
                    <Frame
                        @sizing=ItemSize::Percent(100.0)
                        align=Align::Center
                        padding=MESSAGE_PADDING
                    >
                        <List spacing=MESSAGE_SPACING @test_id={"paint_review.failed"}>
                            <Text
                                style=TextStyle::HEADING
                                color=TextColor::Danger
                                align=beui::TextAlign::Center
                            >
                                "The paintings could not be loaded"
                            </Text>
                            <Text style=TextStyle::CAPTION align=beui::TextAlign::Center>
                                {reason}
                            </Text>
                        </List>
                    </Frame>
                </Show>
                <Show condition={browsing}>
                    <Gallery @sizing=ItemSize::Percent(100.0) review={review} />
                </Show>
                <Show condition={opened}>
                    <Viewer @sizing=ItemSize::Percent(100.0) review={review} />
                </Show>
            </List>
        </Frame>
    }
}

fn shortcut(review: Review, press: KeyPress) -> bool {
    let modifiers = press.modifiers;
    if modifiers.ctrl || modifiers.alt || modifiers.logo || focus_takes_text() {
        return false;
    }
    if press.key == Key::Space {
        if !press.repeat {
            review.space.set(press.pressed);
        }
        return true;
    }
    if !press.pressed {
        return false;
    }
    let open = review.open.get_untracked().is_some();
    match press.key {
        Key::Escape if open => review.close(),
        Key::J if open => review.step_painting(1),
        Key::K if open => review.step_painting(-1),
        Key::Period if open => review.step_frame(1),
        Key::Comma if open => review.step_frame(-1),
        _ => return false,
    }
    true
}

#[component]
fn Shortcuts(hints: Vec<(&'static str, &'static str)>) -> NodeId {
    let hints: Children<ListChild> = hints
        .into_iter()
        .map(|(keys, description)| {
            view! {
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                    <Chip label={keys.to_owned()} />
                    <Text style=TextStyle::CAPTION>{description.to_owned()}</Text>
                </List>
            }
        })
        .collect();
    view! {
        <Frame padding=HINT_PADDING>
            <List
                direction=Direction::Horizontal
                align=Align::Center
                spacing=Space::Xl
                wrap=true
                @test_id={"paint_review.shortcuts"}
            >
                {hints}
            </List>
        </Frame>
    }
}

#[cfg(test)]
mod tests;
