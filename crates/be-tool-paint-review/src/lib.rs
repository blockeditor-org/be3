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
use beui::{Align, Direction};
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
        KeyReveal::Focus,
        move |event: KeyEvent| event.press().is_some_and(|press| shortcut(review, press)),
    );
    let reason = create_memo(move || match manifest.get() {
        Fetch::Failed(error) => error,
        _ => String::new(),
    });
    view! {
        <Frame fill=ColorRole::Background>
            <List spacing=Space::None>
                <Match
                    value={manifest}
                    fallback={move || view! {
                        <ListItem grow=1.0>
                            <Frame align=Align::Center padding=MESSAGE_PADDING>
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
                        </ListItem>
                    }}
                >
                    <Case is={Fetch::Loading}>
                        <ListItem grow=1.0>
                            <Frame align=Align::Center>
                                <List
                                    align=Align::Center
                                    spacing=MESSAGE_SPACING
                                    @test_id={"paint_review.loading"}
                                >
                                    <Progress
                                        width=INLINE_PROGRESS_WIDTH
                                        label="Loading the paintings"
                                    />
                                    <Text style=TextStyle::CAPTION>"Loading the paintings"</Text>
                                </List>
                            </Frame>
                        </ListItem>
                    </Case>
                    <Case
                        when={|manifest: &Fetch<Rc<Manifest>>| matches!(manifest, Fetch::Ready(_))}
                    >
                        <Show
                            value={move || review.open.with(Option::is_some)}
                            fallback={move || view! {
                                <ListItem grow=1.0>
                                    <Gallery review={review} />
                                </ListItem>
                            }}
                        >
                            <ListItem grow=1.0>
                                <Viewer review={review} />
                            </ListItem>
                        </Show>
                    </Case>
                </Match>
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
    let hints: Children<ListItem> = hints
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
