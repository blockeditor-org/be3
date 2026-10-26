use beui::Wrap;
use std::rc::Rc;

use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::{DockPane, DockSplit, DockTab};
use beui::{Context, Document, Key, KeyPress, NodeId, Rect};

mod images;
mod painting;
mod review;
mod sidebar;
mod stage;

pub use images::{ImageRequest, ImageState, Images};

use paint_review_manifest::Manifest;
use painting::PaintingView;
use review::{Mode, Review};
use sidebar::PaintingList;

const MESSAGE_SPACING: f32 = 8.0;
const MESSAGE_PADDING: f32 = 24.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Fetch<T> {
    Loading,
    Ready(T),
    Failed(String),
}

pub struct PaintReviewApp {
    pub document: Document,
    manifest: OwnedSignal<Fetch<Rc<Manifest>>>,
    images: OwnedStored<images::ImageTable>,
}

impl PaintReviewApp {
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

impl beui::App for PaintReviewApp {
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
    let initial = painting.get_untracked();
    let focus = if initial.is_some() { 2u64 } else { 1u64 };
    let review = Review::new(manifest, images, initial);
    let layout = review.layout;
    on_shortcut(move |press: KeyPress| shortcut(review, press));
    let loading = create_memo(move || manifest.with(|manifest| matches!(manifest, Fetch::Loading)));
    let failure = create_memo(move || match manifest.get() {
        Fetch::Failed(error) => Some(error),
        _ => None,
    });
    let failed = create_memo(move || failure.get().is_some());
    let reason = create_memo(move || failure.get().unwrap_or_default());
    let ready = create_memo(move || manifest.with(|manifest| matches!(manifest, Fetch::Ready(_))));
    view! {
        <Frame fill=ColorRole::Background>
            <List spacing=0.0>
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
                        padding={Sides::all(MESSAGE_PADDING)}
                    >
                        <List spacing=MESSAGE_SPACING @test_id={"paint_review.failed"}>
                            <Text
                                style=TextStyle::HEADING
                                color=TextColor::Danger
                                align=beui::TextAlign::Center
                            >
                                "The paintings could not be loaded"
                            </Text>
                            <Text
                                style=TextStyle::CAPTION
                                wrap=Wrap::Words
                                align=beui::TextAlign::Center
                            >
                                {reason}
                            </Text>
                        </List>
                    </Frame>
                </Show>
                <Show condition={ready}>
                    <Docking @sizing=ItemSize::Percent(100.0) layout focus>
                        <DockSplit id="review" fraction=0.28>
                            <DockPane id="paintings">
                                <DockTab id=1u64 title="Paintings">
                                    <PaintingList review={review} />
                                </DockTab>
                            </DockPane>
                            <DockPane id="painting">
                                <DockTab id=2u64 title="Painting">
                                    <PaintingView review={review} />
                                </DockTab>
                            </DockPane>
                        </DockSplit>
                    </Docking>
                </Show>
            </List>
        </Frame>
    }
}

fn shortcut(review: Review, press: KeyPress) -> bool {
    let modifiers = press.modifiers;
    if !press.pressed || modifiers.ctrl || modifiers.alt || modifiers.logo || focus_takes_text() {
        return false;
    }
    let mode = |mode: Mode| {
        review.choose(mode);
        true
    };
    match press.key {
        Key::Space => {
            if !press.repeat {
                review.flip();
            }
            true
        }
        Key::J => {
            review.step_painting(1);
            true
        }
        Key::K => {
            review.step_painting(-1);
            true
        }
        Key::Period => {
            review.step_frame(1);
            true
        }
        Key::Comma => {
            review.step_frame(-1);
            true
        }
        Key::One => mode(Mode::Before),
        Key::Two => mode(Mode::After),
        Key::Three => mode(Mode::Changes),
        Key::Four => mode(Mode::SideBySide),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
