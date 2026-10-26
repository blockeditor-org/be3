use beui::Wrap;
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::{Container, PanZoom, PanZoomHandle, PanZoomView, Picture};
use beui::{ImageFit, NodeId, Vec2, pos2};

use crate::images::ImageState;
use crate::review::{Mode, Review};

const GAP: f32 = 24.0;
const MESSAGE_PADDING: f32 = 24.0;
const MESSAGE_SPACING: f32 = 8.0;

#[component]
pub(crate) fn Stage(review: Review) -> NodeId {
    let images = review.images;
    let slots = create_memo(move || {
        let mode = review.mode.get();
        let hashes = review.frame.with(|frame| match (frame, mode) {
            (Some(frame), Some(mode)) => mode.images(frame),
            _ => Vec::new(),
        });
        hashes
            .into_iter()
            .map(|hash| images.state(&hash).get())
            .collect::<Vec<ImageState>>()
    });
    let sizes = create_memo(move || {
        slots.with(|slots| {
            slots
                .iter()
                .map(|slot| match slot {
                    ImageState::Ready(image) => Some(image.size()),
                    _ => None,
                })
                .collect::<Vec<Option<Vec2>>>()
        })
    });
    let content = create_memo(move || {
        let sizes = sizes.get();
        let sizes: Option<Vec<Vec2>> = sizes.into_iter().collect();
        let sizes = sizes.filter(|sizes| !sizes.is_empty())?;
        let width = sizes.iter().map(|size| size.x).sum::<f32>()
            + GAP * sizes.len().saturating_sub(1) as f32;
        let height = sizes.iter().map(|size| size.y).fold(0.0, f32::max);
        Some((width, height))
    });
    let centering = create_memo(move || (review.selected.get(), content.get()));
    let camera = review.camera;
    let fit_request = create_signal(0u64);

    let loading = create_memo(move || {
        slots.with(|slots| slots.iter().any(|slot| matches!(slot, ImageState::Loading)))
    });
    let failure = create_memo(move || {
        slots.with(|slots| {
            slots.iter().find_map(|slot| match slot {
                ImageState::Failed(error) => {
                    Some(format!("The image could not be loaded: {error}"))
                }
                _ => None,
            })
        })
    });
    let failed = create_memo(move || failure.get().is_some());
    let reason = create_memo(move || failure.get().unwrap_or_default());
    let empty = create_memo(move || slots.with(Vec::is_empty));
    let side_by_side = create_memo(move || review.mode.get() == Some(Mode::SideBySide));
    let places = create_memo(move || (0..slots.with(Vec::len)).collect::<Vec<usize>>());

    view! {
        <Frame fill=ColorRole::Surface @test_id={"paint_review.stage"}>
            <Container>|size: ReadSignal<Vec2>| {
                create_effect(move || {
                    let (_, content) = centering.get();
                    fit_request.get();
                    let available = size.get();
                    if let Some((width, height)) = content
                        && available.x > 0.0 && available.y > 0.0 {
                            let scale = ((available.x - 24.0).max(1.0) / width)
                                .min((available.y - 24.0).max(1.0) / height)
                                .clamp(0.05, 1.0);
                            camera.set(PanZoomView::new(pos2(width / 2.0, height / 2.0), scale));
                        }
                });
                view! {
                    <Layers>
                        <PanZoom
                            view={camera}
                            @test_id="paint_review.pan_zoom"
                        >|handle: PanZoomHandle| {
                            let PanZoomHandle { view, focused, .. } = handle;
                            view! {
                                <Frame border={focus_border(focused, Border::FOCUS)}>
                                    <Canvas view>
                                        <ForEach keys={places}>|index: usize| {
                                            let image = create_memo(move || {
                                                slots.with(|slots| match slots.get(index) {
                                                    Some(ImageState::Ready(image)) => Some(image.clone()),
                                                    _ => None,
                                                })
                                            });
                                            let size = create_memo(move || {
                                                sizes.with(|sizes| sizes.get(index).copied().flatten())
                                                    .unwrap_or(Vec2::ZERO)
                                            });
                                            let x = create_memo(move || {
                                                sizes.with(|sizes| {
                                                    sizes
                                                        .iter()
                                                        .take(index)
                                                        .map(|size| size.map_or(0.0, |size| size.x) + GAP)
                                                        .sum::<f32>()
                                                })
                                            });
                                            let width = create_memo(move || size.get().x);
                                            let height = create_memo(move || size.get().y);
                                            view! {
                                                <CanvasItem
                                                    x={x}
                                                    y=0.0
                                                    width={width}
                                                    height={height}
                                                >
                                                    <Picture
                                                        image={image}
                                                        fit=ImageFit::Fill
                                                        smooth=false
                                                    />
                                                </CanvasItem>
                                            }
                                        }</ForEach>
                                    </Canvas>
                                </Frame>
                            }
                        }</PanZoom>
                        <Show condition={side_by_side}>
                            <Frame align={Align2::new(Align::Center, Align::Start)}>
                                <Frame padding={Sides::all(MESSAGE_PADDING / 2.0)}>
                                    <Text style=TextStyle::CAPTION>
                                        "Before on the left, after on the right"
                                    </Text>
                                </Frame>
                            </Frame>
                        </Show>
                        <Frame align=Align::Center padding={Sides::all(MESSAGE_PADDING)}>
                            <List spacing=0.0>
                                <Show condition={loading}>
                                    <List
                                        align=Align::Center
                                        spacing=MESSAGE_SPACING
                                        @test_id={"paint_review.image.loading"}
                                    >
                                        <Progress
                                            width=INLINE_PROGRESS_WIDTH
                                            label="Loading the image"
                                        />
                                        <Text style=TextStyle::CAPTION>"Loading the image"</Text>
                                    </List>
                                </Show>
                                <Show condition={empty}>
                                    <Text
                                        style=TextStyle::PARAGRAPH
                                        @test_id={"paint_review.image.none"}
                                    >
                                        "There is no image of this painting to show"
                                    </Text>
                                </Show>
                                <Show condition={failed}>
                                    <Text
                                        color=TextColor::Danger
                                        wrap=Wrap::Words
                                        @test_id={"paint_review.image.failed"}
                                    >
                                        {reason}
                                    </Text>
                                </Show>
                            </List>
                        </Frame>
                        <Frame
                            align={Align2::new(Align::End, Align::End)}
                            padding={Sides::all(12.0)}
                        >
                            <List direction=Direction::Horizontal spacing=8.0>
                                <Button
                                    variant=ButtonVariant::Secondary
                                    label="Fit"
                                    @test_id="paint_review.fit"
                                    on_click={move || fit_request.update(|n| *n += 1)}
                                />
                                <Button
                                    variant=ButtonVariant::Secondary
                                    label="100%"
                                    @test_id="paint_review.actual_size"
                                    on_click={move || camera.update(|view| view.scale = 1.0)}
                                />
                            </List>
                        </Frame>
                    </Layers>
                }
            }</Container>
        </Frame>
    }
}
