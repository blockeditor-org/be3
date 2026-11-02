use beui::Overflow;
use beui::icons::{ICON_ARROW_LEFT, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT};
use beui::primitives::Interactive;
use beui::reactive::*;
use beui::styled::*;
use beui::{
    Align, Align2, CursorIcon, Direction, ImageFit, ItemSize, NodeId, PointerPress, Rect, TextAlign,
    Vec2, pos2,
};

use crate::Shortcuts;
use crate::images::ImageState;
use crate::review::{Review, Side, missing};
use crate::status::{color, glyph, label, role};
use be_tool_paint_review_manifest::{Frame, Picture, Status};

const PADDING: Space = Space::Xl;
const SPACING: Space = Space::Md;
const BADGE_PADDING: Padding = Padding::symmetric(Space::Md, Space::Xs2);
const FIT_MARGIN: f32 = 24.0;
const HANDLE_WIDTH: f32 = 24.0;
const HANDLE_LINE: f32 = 2.0;
const HINTS: [(&str, &str); 5] = [
    ("Drag the line", "compare"),
    ("Hold Space", "before"),
    ("Hold Shift", "changes"),
    ("J K", "next, previous painting"),
    ("Esc", "all paintings"),
];

#[derive(Clone, Debug, PartialEq)]
enum Shown {
    Compare(Picture, Picture),
    One(Picture),
    Missing(String),
}

impl Shown {
    fn of(frame: &Frame, status: Status, held: Option<Side>) -> Self {
        let side = match (held, &frame.before, &frame.after) {
            (Some(side), _, _) => side,
            (None, Some(before), Some(after)) => {
                return Shown::Compare(before.clone(), after.clone());
            }
            (None, Some(_), None) => Side::Before,
            (None, _, _) => Side::After,
        };
        match side.of(frame) {
            Some(picture) => Shown::One(picture.clone()),
            None => Shown::Missing(missing(status, side).to_owned()),
        }
    }

    fn pictures(&self) -> Vec<&Picture> {
        match self {
            Shown::Compare(before, after) => vec![before, after],
            Shown::One(picture) => vec![picture],
            Shown::Missing(_) => Vec::new(),
        }
    }

    fn size(&self) -> Option<Vec2> {
        let pictures = self.pictures();
        if pictures.is_empty() {
            return None;
        }
        Some(pictures.iter().fold(Vec2::ZERO, |size, picture| {
            size.max(Vec2::new(picture.width as f32, picture.height as f32))
        }))
    }
}

#[component]
pub(crate) fn Viewer(review: Review) -> NodeId {
    view! {
        <List spacing=Space::None>
            <Header review={review} />
            <Controls review={review} />
            <Separator />
            <Stage @sizing=ItemSize::Percent(100.0) review={review} />
            <Separator />
            <Shortcuts hints={HINTS.to_vec()} />
        </List>
    }
}

#[component]
fn Header(review: Review) -> NodeId {
    let name = create_memo(move || {
        review.painting.with(|painting| {
            painting
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_default()
        })
    });
    let status = create_memo(move || {
        review
            .painting
            .with(|painting| painting.as_ref().map_or(Status::Changed, |p| p.status))
    });
    let detail = create_memo(move || {
        review.painting.with(|painting| {
            painting
                .as_ref()
                .map(|p| p.detail.clone())
                .unwrap_or_default()
        })
    });
    let detailed = create_memo(move || !detail.get().is_empty());
    let position = create_memo(move || {
        format!(
            "{} of {}",
            review.open.get().map_or(0, |index| index + 1),
            review.count.get()
        )
    });
    view! {
        <Frame padding={Padding::all(PADDING).bottom(SPACING)}>
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Button
                        variant=ButtonVariant::Secondary
                        glyph=ICON_ARROW_LEFT
                        label="All paintings"
                        @test_id="paint_review.back"
                        on_click={move || review.close()}
                    />
                    <Text
                        @sizing={ItemSize::Intrinsic.shrink(1.0)}
                        style=TextStyle::HEADING
                        family=Family::Mono
                        overflow=Overflow::Ellipsis
                        @test_id={"paint_review.name"}
                    >
                        {name}
                    </Text>
                    <Dynamic value={status}>|status: Status|
                        <StatusBadge status />
                    </Dynamic>
                    <Text style=TextStyle::CAPTION @test_id={"paint_review.position"}>
                        {position}
                    </Text>
                </List>
                <Show condition={detailed}>
                    <Text style=TextStyle::CAPTION @test_id={"paint_review.detail"}>{detail}</Text>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn StatusBadge(status: Status) -> NodeId {
    view! {
        <Frame
            border={Some(Border::new(role(status)))}
            radius=Radius::Full
            padding=BADGE_PADDING
            @test_id={"paint_review.status"}
        >
            <Text size=TextSize::Sm color={color(status)} align=TextAlign::Center>
                <Icon glyph={glyph(status).to_owned()} />
                {label(status).to_owned()}
            </Text>
        </Frame>
    }
}

#[component]
fn Controls(review: Review) -> NodeId {
    let count = create_memo(move || review.frame_count());
    let many = create_memo(move || count.get() > 1);
    let first = create_memo(move || review.frame_index.get() == 0);
    let last = create_memo(move || review.frame_index.get() + 1 >= count.get());
    let position = create_memo(move || {
        let number = review
            .frame
            .with(|frame| frame.as_ref().map_or(0, |frame| frame.number));
        format!(
            "Frame {number}, {} of {} that changed",
            review.frame_index.get() + 1,
            count.get()
        )
    });
    let pixels = create_memo(move || {
        review.frame.with(
            |frame| match frame.as_ref().and_then(|frame| frame.changed_pixels) {
                Some(1) => "1 pixel differs".to_owned(),
                Some(count) => format!("{count} pixels differ"),
                None => String::new(),
            },
        )
    });
    let counted = create_memo(move || !pixels.get().is_empty());
    let showing = create_memo(move || match (review.holding.get(), review.side.get()) {
        (false, _) => "Showing before and after".to_owned(),
        (true, side) => format!("Showing {}", side.label().to_lowercase()),
    });
    view! {
        <Frame padding={Padding::horizontal(PADDING).bottom(SPACING)}>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING wrap=true>
                <Text style=TextStyle::CAPTION @test_id={"paint_review.showing"}>{showing}</Text>
                <Show condition={counted}>
                    <Text style=TextStyle::CAPTION @test_id={"paint_review.pixels"}>{pixels}</Text>
                </Show>
                <Show condition={many}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Button
                            icon_only=true
                            glyph=ICON_CHEVRON_LEFT
                            label="The frame before"
                            disabled={first}
                            @test_id={"paint_review.frame.previous"}
                            on_click={move || review.step_frame(-1)}
                        />
                        <Text style=TextStyle::CAPTION @test_id={"paint_review.frame"}>
                            {position}
                        </Text>
                        <Button
                            icon_only=true
                            glyph=ICON_CHEVRON_RIGHT
                            label="The frame after"
                            disabled={last}
                            @test_id={"paint_review.frame.next"}
                            on_click={move || review.step_frame(1)}
                        />
                    </List>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn Stage(review: Review) -> NodeId {
    let images = review.images;
    let shown = create_memo(move || {
        let held = review.holding.get().then(|| review.side.get());
        let status = review
            .painting
            .with(|painting| painting.as_ref().map_or(Status::Unreadable, |p| p.status));
        review.frame.with(|frame| match frame {
            Some(frame) => Shown::of(frame, status, held),
            None => Shown::Missing(missing(status, Side::After).to_owned()),
        })
    });
    let states = create_memo(move || {
        shown.with(|shown| {
            shown
                .pictures()
                .into_iter()
                .map(|picture| images.state(&picture.image).get())
                .collect::<Vec<ImageState>>()
        })
    });
    let content = create_memo(move || shown.with(Shown::size));
    let comparing = create_memo(move || matches!(shown.get(), Shown::Compare(..)));
    let notice = create_memo(move || match shown.get() {
        Shown::Missing(notice) => notice,
        _ => String::new(),
    });
    let noticed = create_memo(move || !notice.get().is_empty());
    let loading = create_memo(move || {
        states.with(|states| states.iter().any(|state| matches!(state, ImageState::Loading)))
    });
    let failure = create_memo(move || {
        states.with(|states| {
            states.iter().find_map(|state| match state {
                ImageState::Failed(error) => {
                    Some(format!("The image could not be loaded: {error}"))
                }
                _ => None,
            })
        })
    });
    let failed = create_memo(move || failure.get().is_some());
    let reason = create_memo(move || failure.get().unwrap_or_default());
    let camera = review.camera;
    let fit_request = create_signal(0u64);
    let centering = create_memo(move || (review.open.get(), content.get()));

    let image_of = move |which: usize| {
        create_memo(move || match states.with(|states| states.get(which).cloned()) {
            Some(ImageState::Ready(image)) => Some(image),
            _ => None,
        })
    };
    let first = image_of(0);
    let second = image_of(1);
    let first_size = create_memo(move || {
        shown.with(|shown| {
            shown
                .pictures()
                .first()
                .map_or(Vec2::ZERO, |p| Vec2::new(p.width as f32, p.height as f32))
        })
    });
    let second_size = create_memo(move || {
        shown.with(|shown| {
            shown
                .pictures()
                .get(1)
                .map_or(Vec2::ZERO, |p| Vec2::new(p.width as f32, p.height as f32))
        })
    });
    let split = review.split;
    let split_x = create_memo(move || split.get() * content.get().map_or(0.0, |size| size.x));
    let second_x = create_memo(move || split_x.get().min(second_size.get().x));
    let second_width = create_memo(move || (second_size.get().x - second_x.get()).max(0.0));
    let second_source = create_memo(move || {
        let width = second_size.get().x.max(1.0);
        Some(Rect::from_min_max(
            pos2(second_x.get() / width, 0.0),
            pos2(1.0, 1.0),
        ))
    });
    let height = create_memo(move || content.get().map_or(0.0, |size| size.y));

    view! {
        <Frame fill=ColorRole::Surface @test_id={"paint_review.stage"}>
            <Container>|size: ReadSignal<Vec2>| {
                create_effect(move || {
                    let (_, content) = centering.get();
                    fit_request.get();
                    let available = size.get();
                    if let Some(content) = content
                        && available.x > 0.0 && available.y > 0.0 {
                            let scale = ((available.x - FIT_MARGIN).max(1.0) / content.x)
                                .min((available.y - FIT_MARGIN).max(1.0) / content.y)
                                .clamp(0.05, 1.0);
                            camera.set(PanZoomView::new(pos2(content.x / 2.0, content.y / 2.0), scale));
                        }
                });
                view! {
                    <Layers>
                        <PanZoom view={camera} @test_id="paint_review.pan_zoom">|handle: PanZoomHandle| {
                            let PanZoomHandle { view, scale, focused, .. } = handle;
                            let handle_x = create_memo(move || {
                                split_x.get() - HANDLE_WIDTH / 2.0 / scale.get().max(0.01)
                            });
                            let handle_width = create_memo(move || HANDLE_WIDTH / scale.get().max(0.01));
                            let drag = move |press: PointerPress| {
                                let (Some(view), Some(content)) = (view.get_untracked(), content.get_untracked()) else {
                                    return;
                                };
                                let x = view.to_canvas(press.pos).x;
                                split.set((x / content.x.max(1.0)).clamp(0.0, 1.0));
                            };
                            view! {
                                <Frame border={focus_border(focused, Border::FOCUS)}>
                                    <Canvas view>
                                        <CanvasItem
                                            x=0.0
                                            y=0.0
                                            width={create_memo(move || first_size.get().x)}
                                            height={create_memo(move || first_size.get().y)}
                                        >
                                            <Picture image={first} fit=ImageFit::Fill smooth=false />
                                        </CanvasItem>
                                        <CanvasItem
                                            x={second_x}
                                            y=0.0
                                            width={second_width}
                                            height={create_memo(move || second_size.get().y)}
                                        >
                                            <Picture
                                                image={second}
                                                source={second_source}
                                                fit=ImageFit::Fill
                                                smooth=false
                                            />
                                        </CanvasItem>
                                        <CanvasItem
                                            x={handle_x}
                                            y=0.0
                                            width={create_memo(move || if comparing.get() { handle_width.get() } else { 0.0 })}
                                            height={height}
                                        >
                                            <Interactive
                                                cursor={Some(CursorIcon::ResizeColumn)}
                                                on_press={drag}
                                                on_drag={drag}
                                                @test_id="paint_review.split"
                                            >
                                                <Frame align={Align2::new(Align::Center, Align::Stretch)}>
                                                    <Frame
                                                        width={Size::Custom(HANDLE_LINE)}
                                                        height=Length::FILL
                                                        fill=ColorRole::Accent
                                                    />
                                                </Frame>
                                            </Interactive>
                                        </CanvasItem>
                                    </Canvas>
                                </Frame>
                            }
                        }</PanZoom>
                        <Show condition={comparing}>
                            <Frame align={Align2::new(Align::Start, Align::Start)} padding=Space::Lg>
                                <Text style=TextStyle::CAPTION>"Before"</Text>
                            </Frame>
                        </Show>
                        <Show condition={comparing}>
                            <Frame align={Align2::new(Align::End, Align::Start)} padding=Space::Lg>
                                <Text style=TextStyle::CAPTION>"After"</Text>
                            </Frame>
                        </Show>
                        <Frame align=Align::Center padding=Space::Xl3>
                            <List spacing=Space::None>
                                <Show condition={loading}>
                                    <List
                                        align=Align::Center
                                        spacing=SPACING
                                        @test_id={"paint_review.image.loading"}
                                    >
                                        <Progress width=INLINE_PROGRESS_WIDTH label="Loading the image" />
                                        <Text style=TextStyle::CAPTION>"Loading the image"</Text>
                                    </List>
                                </Show>
                                <Show condition={noticed}>
                                    <Text
                                        style=TextStyle::HEADING
                                        @test_id={"paint_review.image.none"}
                                    >
                                        {notice}
                                    </Text>
                                </Show>
                                <Show condition={failed}>
                                    <Text color=TextColor::Danger @test_id={"paint_review.image.failed"}>
                                        {reason}
                                    </Text>
                                </Show>
                            </List>
                        </Frame>
                        <Frame align={Align2::new(Align::End, Align::End)} padding=Space::Lg>
                            <List direction=Direction::Horizontal spacing=SPACING>
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
